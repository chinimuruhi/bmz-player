use super::*;

#[test]
fn battle_skin_headers_receive_standard_play_offsets() {
    for skin_type in [12, 13] {
        let offsets =
            skin_offset_definitions_from_header(&serde_json::json!({ "type": skin_type }));

        assert!(offsets.iter().any(|(_, id)| *id == 10));
        assert!(offsets.iter().any(|(_, id)| *id == 30));
        assert!(offsets.iter().any(|(_, id)| *id == 32));
        assert!(offsets.iter().any(|(_, id)| *id == 33));
    }
}

#[test]
fn infers_select_score_availability_from_luxe_global_guard() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let draw = lua
        .load("flag_score = false; return function() return flag_score end")
        .eval::<Function>()
        .unwrap();

    assert_eq!(
        infer_select_score_available_draw_condition(&lua, &draw, &probe).as_deref(),
        Some("select_score_available()")
    );
    assert!(!lua.globals().get::<bool>("flag_score").unwrap());
}

#[test]
fn infers_select_score_availability_from_mz_select_local_guard() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let draw = lua
        .load("local flag_score = false; return function() return flag_score end")
        .eval::<Function>()
        .unwrap();

    assert_eq!(
        infer_select_score_available_draw_condition(&lua, &draw, &probe).as_deref(),
        Some("select_score_available()")
    );
    assert!(!draw.call::<bool>(()).unwrap());
}

#[test]
fn load_constant_fallback_preserves_existing_stub_behavior() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    let draw = lua
        .load(
            r#"return function()
                    local ex = main_state.number(71)
                    local max = main_state.number(74) * 2
                    if max == 0 then return false end
                    local rate = ex / max
                    return rate >= 2 / 9 and rate < 3 / 9
                end"#,
        )
        .eval::<Function>()
        .unwrap();
    let value = lua
        .load(
            r#"return function()
                    local ex = main_state.number(71)
                    local max = main_state.number(74) * 2
                    if max == 0 then return 0 end
                    return math.abs(ex - math.ceil(max * 8 / 9))
                end"#,
        )
        .eval::<Function>()
        .unwrap();
    let timer_value =
        lua.load("return function() return main_state.time() end").eval::<Function>().unwrap();
    let constant = lua.load("return function() return 42 end").eval::<Function>().unwrap();

    assert!(infer_constant_draw_at_load(&draw, &probe).is_some());
    assert!(infer_constant_number_at_load(&value, &probe).is_some());
    assert!(infer_constant_number_at_load(&timer_value, &probe).is_some());
    assert_eq!(infer_constant_number_at_load(&constant, &probe).as_deref(), Some("42"));
}

#[test]
fn infers_wmii_result_score_runtime_expressions() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    let functions = lua
        .load(
            r#"
                local ranks = {
                    {name="F", value=0/9}, {name="E", value=2/9},
                    {name="D", value=3/9}, {name="C", value=4/9},
                    {name="B", value=5/9}, {name="A", value=6/9},
                    {name="AA", value=7/9}, {name="AAA", value=8/9},
                    {name="MAX", value=1},
                }
                local function info()
                    local ex = main_state.number(71)
                    local max = main_state.number(74) * 2
                    if max == 0 then return nil end
                    if ex >= max then return {target="MAX", sign="+", diff=0} end
                    local current = 1
                    for i = 1, #ranks do
                        if ex / max >= ranks[i].value then current = i else break end
                    end
                    local cur, next = ranks[current], ranks[current + 1]
                    local lower = math.ceil(cur.value * max)
                    local upper = math.ceil(next.value * max)
                    local to_lower = math.max(0, ex - lower)
                    local to_upper = math.max(0, upper - ex)
                    if to_lower <= to_upper then
                        return {target=cur.name, sign="+", diff=to_lower}
                    end
                    return {target=next.name, sign="-", diff=to_upper}
                end
                return {
                    band = function()
                        local ex = main_state.number(71)
                        local max = main_state.number(74) * 2
                        if max == 0 then return false end
                        return ex / max >= 2/9 and ex / max < 3/9
                    end,
                    max = function()
                        local ex = main_state.number(71)
                        local max = main_state.number(74) * 2
                        if max == 0 then return false end
                        return ex / max == 1
                    end,
                    diff = function() local i=info(); return i and i.diff or 0 end,
                    luxe_diff = function()
                        local ex = main_state.number(71)
                        local max = main_state.number(74) * 2
                        local _best = main_state.number(170)
                        local _rival = main_state.number(271)
                        if max <= 0 or ex >= max then return 0 end
                        local boundaries = {0, 2, 3, 4, 5, 6, 7, 8, 9}
                        local current = 1
                        for i = 1, #boundaries do
                            if ex * 9 >= boundaries[i] * max then current = i else break end
                        end
                        local lower, upper = boundaries[current], boundaries[current + 1]
                        local lower_score = math.ceil(lower * max / 9)
                        local upper_score = math.ceil(upper * max / 9)
                        if ex * 18 < (lower + upper) * max then
                            return math.max(0, ex - lower_score)
                        end
                        return math.max(0, upper_score - ex)
                    end,
                    aaa_minus = function()
                        local i=info(); return i and i.target == "AAA" and i.sign == "-"
                    end,
                    plus = function() local i=info(); return i and i.sign == "+" end,
                    text = function() return main_state.text(1001).." "..main_state.text(1002) end,
                }
                "#,
        )
        .eval::<Table>()
        .unwrap();

    assert_eq!(
        infer_score_rate_band(&functions.get::<Function>("band").unwrap(), &probe).as_deref(),
        Some("score_rate_band(2,3)")
    );
    assert_eq!(
        infer_score_rate_band(&functions.get::<Function>("max").unwrap(), &probe).as_deref(),
        Some("score_rate_band(9,10)")
    );
    assert_eq!(
        infer_nearest_rank_diff_value_expr(
            &functions.get::<Function>("diff").unwrap(),
            Some("diff_rank"),
            &probe,
        )
        .as_deref(),
        Some("bmz:nearest_rank_diff_abs")
    );
    assert_eq!(
        infer_nearest_rank_diff_value_expr(
            &functions.get::<Function>("luxe_diff").unwrap(),
            Some("rank_diff_count"),
            &probe,
        )
        .as_deref(),
        Some("bmz:nearest_rank_diff_abs")
    );
    assert_eq!(
        infer_result_score_draw(
            &functions.get::<Function>("aaa_minus").unwrap(),
            Some("nextRankAAA"),
            &probe,
        )
        .as_deref(),
        Some("nearest_rank(AAA,minus)")
    );
    assert_eq!(
        infer_result_score_draw(
            &functions.get::<Function>("plus").unwrap(),
            Some("diff_plus"),
            &probe,
        )
        .as_deref(),
        Some("nearest_rank_sign(plus)")
    );
    assert_eq!(
        infer_result_score_draw(
            &functions.get::<Function>("plus").unwrap(),
            Some("rank_diff_aaa_plus"),
            &probe,
        )
        .as_deref(),
        Some("nearest_rank(AAA,plus)")
    );
    assert_eq!(
        infer_text_concat_expr(&functions.get::<Function>("text").unwrap(), &probe).as_deref(),
        Some("bmz:text_concat:1001:1002")
    );
}

#[test]
fn infers_wmii_result_ir_ranking_runtime_expressions() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    lua.globals().set("Expand_op", 1).unwrap();
    let functions = lua
        .load(
            r#"
                return {
                    graph = function()
                        return main_state.number(382) / (main_state.number(74) * 2)
                    end,
                    rate_integer = function()
                        local score = main_state.number(382)
                        local max = main_state.number(74) * 2
                        if score > 0 and max > 0 then return math.floor(score / max * 100) end
                        return 0
                    end,
                    rate_fraction = function()
                        local score = main_state.number(382)
                        local max = main_state.number(74) * 2
                        if score > 0 and max > 0 then return (score / max * 10000) % 100 end
                        return 0
                    end,
                    diff = function()
                        return math.max(main_state.number(170), main_state.number(171))
                            - main_state.number(382)
                    end,
                    band = function()
                        local rate = main_state.number(382) / (main_state.number(74) * 2)
                        return rate >= 7/9 and rate < 8/9 and Expand_op == 1
                    end,
                    name = function()
                        local current = main_state.text(122)
                        local own = main_state.text(1021)
                        if current == own then return own end
                        return main_state.text(122)
                    end,
                    own = function()
                        return main_state.text(122) == main_state.text(1021) and Expand_op == 1
                    end,
                }
                "#,
        )
        .eval::<Table>()
        .unwrap();

    assert_eq!(
        infer_ir_ranking_score_value_expr(
            &functions.get::<Function>("graph").unwrap(),
            Some("ir_scoreGraph3"),
            &probe,
        )
        .as_deref(),
        Some("bmz:ir_score_rate:3")
    );
    assert_eq!(
        infer_ir_ranking_score_rate_value_expr(
            &functions.get::<Function>("rate_integer").unwrap(),
            Some("ir_scorerate3"),
            &probe,
        )
        .as_deref(),
        Some("bmz:ir_score_rate_integer:3")
    );
    assert_eq!(
        infer_ir_ranking_score_rate_value_expr(
            &functions.get::<Function>("rate_fraction").unwrap(),
            Some("ir_scorerate_dot3"),
            &probe,
        )
        .as_deref(),
        Some("bmz:ir_score_rate_fraction:3")
    );
    assert_eq!(
        infer_ir_ranking_score_diff_value_expr(
            &functions.get::<Function>("diff").unwrap(),
            Some("ir_diff_score3"),
            &probe,
        )
        .as_deref(),
        Some("bmz:ir_score_diff:3")
    );
    assert_eq!(
        infer_result_score_draw(
            &functions.get::<Function>("band").unwrap(),
            Some("ir_scoreGraph3"),
            &probe,
        )
        .as_deref(),
        Some("ir_score_rate_band(3,7,8)")
    );
    assert_eq!(
        infer_ir_ranking_name_ref(
            &functions.get::<Function>("name").unwrap(),
            Some("ir_username3"),
            &probe,
        ),
        Some(122)
    );
    assert_eq!(
        infer_result_score_draw(
            &functions.get::<Function>("own").unwrap(),
            Some("irYouFrame"),
            &probe,
        )
        .as_deref(),
        Some("ir_ranking_user(3)")
    );
}

#[test]
fn infers_modern_chic_select_graph_runtime_expressions() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    let functions = lua
        .load(
            r#"
                return {
                    fast = function()
                        local slow = main_state.number(424)
                        local fast = main_state.number(423)
                        return fast / (slow + fast)
                    end,
                    slow = function()
                        local slow = main_state.number(424)
                        local fast = main_state.number(423)
                        return slow / (slow + fast)
                    end,
                    graph = function()
                        local score = main_state.number(380)
                        if score == -2147483648 then return 0 end
                        return score / (main_state.number(74) * 2)
                    end,
                    band = function()
                        local score = main_state.number(380)
                        local rate = (score / (main_state.number(74) * 2)) * 100
                        return main_state.option(51) and rate <= 88.8 and rate > 77.7
                    end,
                }
                "#,
        )
        .eval::<Table>()
        .unwrap();

    assert_eq!(
        infer_value_float_expr(&functions.get::<Function>("fast").unwrap(), &probe).as_deref(),
        Some("(number(423))/(number(423)+number(424))")
    );
    assert_eq!(
        infer_value_float_expr(&functions.get::<Function>("slow").unwrap(), &probe).as_deref(),
        Some("(number(424))/(number(423)+number(424))")
    );
    assert_eq!(
        infer_ir_ranking_score_value_expr(
            &functions.get::<Function>("graph").unwrap(),
            Some("s_rankingGraphAA1"),
            &probe,
        )
        .as_deref(),
        Some("bmz:ir_score_rate:1")
    );
    assert_eq!(
        infer_result_score_draw(
            &functions.get::<Function>("band").unwrap(),
            Some("s_rankingGraphAA1"),
            &probe,
        )
        .as_deref(),
        Some("option(51) and ir_score_rate_range(1,777,888)")
    );
    assert_eq!(modern_chic_ir_ranking_graph("s_rankingGraphAAA10"), Some((10, "AAA")));
}

#[test]
fn infers_wmii_result_panel_gates_without_mutating_default() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    lua.globals().set("Expand_op", 2).unwrap();
    let functions = lua
        .load(
            r#"
                return {
                    ir = function() return Expand_op == 1 end,
                    not_ir = function() return Expand_op ~= 1 end,
                    band = function()
                        local rate = main_state.number(382) / (main_state.number(74) * 2)
                        return rate >= 7/9 and rate < 8/9 and Expand_op == 1
                    end,
                    own = function()
                        return main_state.text(122) == main_state.text(1021) and Expand_op == 1
                    end,
                    timing_negative = function()
                        return (main_state.number(374) + main_state.number(375) * 0.01) < 0
                            and Expand_op == 2
                    end,
                    timing_non_negative = function()
                        return (main_state.number(374) + main_state.number(375) * 0.01) >= 0
                            and Expand_op == 2
                    end,
                }
                "#,
        )
        .eval::<Table>()
        .unwrap();

    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("ir").unwrap(),
            None,
            &probe,
        )
        .as_deref(),
        Some("result_panel(1)")
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("not_ir").unwrap(),
            None,
            &probe,
        )
        .as_deref(),
        Some("result_panel(0) or result_panel(2)")
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("band").unwrap(),
            Some("ir_scoreGraph3"),
            &probe,
        )
        .as_deref(),
        Some("result_panel(1) and ir_score_rate_band(3,7,8)")
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("own").unwrap(),
            Some("irYouFrame"),
            &probe,
        )
        .as_deref(),
        Some("result_panel(1) and ir_ranking_user(3)")
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("timing_negative").unwrap(),
            Some("timingAvg"),
            &probe,
        )
        .as_deref(),
        Some("result_panel(2) and number(374) < 0 or result_panel(2) and number(375) < 0")
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("timing_non_negative").unwrap(),
            Some("timingAvg"),
            &probe,
        )
        .as_deref(),
        Some("result_panel(2) and number(374) >= 0 and number(375) >= 0")
    );
    assert_eq!(lua.globals().get::<i32>("Expand_op").unwrap(), 2);
}

#[test]
fn infers_luxe_flat_local_result_panel_state_without_mutating_default() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    let functions = lua
        .load(
            r#"
                local result_mode = 0
                return {
                    graph_act = function() result_mode = 0 end,
                    ir_act = function() result_mode = 1 end,
                    graph = function() return result_mode == 0 end,
                    ir = function() return result_mode == 1 end,
                    graph_score = function()
                        return result_mode == 0 and main_state.number(71) >= 0
                    end,
                }
                "#,
        )
        .eval::<Table>()
        .unwrap();

    assert_eq!(
        infer_result_panel_act_at_load(
            &lua,
            &functions.get::<Function>("graph_act").unwrap(),
            &probe,
        ),
        Some(i64::from(SKIN_EVENT_RESULT_PANEL_GRAPH))
    );
    assert_eq!(
        infer_result_panel_act_at_load(&lua, &functions.get::<Function>("ir_act").unwrap(), &probe,),
        Some(i64::from(SKIN_EVENT_RESULT_PANEL_IR))
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("graph").unwrap(),
            None,
            &probe,
        )
        .as_deref(),
        Some("result_panel(2)")
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("ir").unwrap(),
            None,
            &probe,
        )
        .as_deref(),
        Some("result_panel(1)")
    );
    assert_eq!(
        infer_result_panel_draw_condition(
            &lua,
            &functions.get::<Function>("graph_score").unwrap(),
            None,
            &probe,
        )
        .as_deref(),
        Some("result_panel(2) and number(71) >= 0")
    );
    assert_eq!(probe.lock().unwrap().result_panel_default, Some(2));
    assert_eq!(
        lua_result_mode_upvalue(&lua, &functions.get::<Function>("graph").unwrap())
            .map(|(_, mode)| mode),
        Some(0)
    );
}

#[test]
fn maps_peacefulplay_keylogger_graph_ids_to_builtin_expressions() {
    assert_eq!(
        keylogger_graph_value_expr_from_id("keylogger-graph-judge-3-good").as_deref(),
        Some("bmz:keylogger_graph:judge:3:good")
    );
    assert_eq!(
        keylogger_graph_value_expr_from_id("keylogger-graph-fastslow-9-fast").as_deref(),
        Some("bmz:keylogger_graph:fastslow:9:fast")
    );
    assert!(keylogger_graph_value_expr_from_id("graph-now").is_none());
}

#[test]
fn maps_milliondollar_fast_slow_graph_ids_to_runtime_expressions() {
    assert_eq!(
        milliondollar_fast_slow_graph_value_expr_from_id("Graph_Totalfastslow_Fast").as_deref(),
        Some(
            "(option(928)*number(423)+(1-option(928))*(number(423)+number(410)))/(number(110)+number(111)+number(112)+number(113)+number(114)+number(420))"
        )
    );
    assert_eq!(
        milliondollar_fast_slow_graph_value_expr_from_id("Graph_Totalfastslow_Slow").as_deref(),
        Some(
            "(option(928)*number(424)+(1-option(928))*(number(424)+number(411)))/(number(110)+number(111)+number(112)+number(113)+number(114)+number(420))"
        )
    );
    assert!(milliondollar_fast_slow_graph_value_expr_from_id("graph-now").is_none());
}

/// Third-party skin baseline.  It is intentionally skipped for clean CI
/// checkouts that do not contain the locally installed skin.
#[test]
fn milliondollar_result_fast_slow_graphs_convert_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/MILLIONDOLLAR/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let loaded = load_lua_skin_value(
        &skin_path,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        &BTreeMap::new(),
    )
    .expect("MILLIONDOLLAR result should convert");
    let messages: Vec<_> = loaded.warnings.iter().map(|warning| warning.message.as_str()).collect();
    assert!(
        !messages.iter().any(|message| {
            message.contains("Graph_Totalfastslow_Fast")
                || message.contains("Graph_Totalfastslow_Slow")
                || (message.contains("graph[") && message.contains("unsupported value"))
        }),
        "MILLIONDOLLAR fast/slow graph values should convert: {messages:?}"
    );
    let document = loaded.value.to_string();
    assert!(document.contains("Graph_Totalfastslow_Fast"));
    assert!(document.contains("option(928)*number(423)"));
    assert!(document.contains("Graph_Totalfastslow_Slow"));
    assert!(document.contains("option(928)*number(424)"));
}

#[test]
fn infers_fixed_delay_timer_function() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    let function = lua
        .load(
            r#"return function()
                    local off = main_state.timer_off_value
                    local source = main_state.timer(143)
                    if source == off then return off end
                    local start = source + 1000000
                    if main_state.time() < start then return off end
                    return start
                end"#,
        )
        .eval::<Function>()
        .unwrap();
    assert_eq!(infer_fixed_delay_timer(&function, &probe), Some((143, 1000)));
}

#[test]
fn infers_custom_timer_alias_function() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    lua.globals().set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap()).unwrap();
    let function =
        lua.load("return function() return main_state.timer(150) end").eval::<Function>().unwrap();

    assert_eq!(infer_custom_timer_alias(&function, &probe), Some(150));
}

#[test]
fn infers_event_index_or_draw_condition() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let function = lua
        .load(
            r#"
                return function()
                    return main_state.event_index(42) == 2 or main_state.event_index(42) == 3
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();

    assert_eq!(
        infer_main_state_event_index_draw_condition(&function, &probe),
        Some("event_index(42) == 2 or event_index(42) == 3".to_string())
    );
}

#[test]
fn infers_extended_arrange_event_index_draw_condition() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let function = lua
        .load(
            r#"
                return function()
                    return main_state.event_index(344) == 10
                        or main_state.event_index(344) == 11
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();

    assert_eq!(
        infer_main_state_event_index_draw_condition(&function, &probe),
        Some("event_index(344) == 10 or event_index(344) == 11".to_string())
    );
}

#[test]
fn infers_event_index_and_dp_side_options_draw_condition() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let random = lua
        .load(
            r#"
                return function()
                    local rnd = main_state.event_index(43)
                    return (rnd == 2 or rnd == 3)
                        and (main_state.option(162) or main_state.option(163))
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();
    let normal = lua
        .load(
            r#"
                return function()
                    return main_state.event_index(43) == 0
                        and (main_state.option(162) or main_state.option(163))
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();
    let extended = lua
        .load(
            r#"
                return function()
                    return main_state.event_index(345) == 11
                        and (main_state.option(162) or main_state.option(163))
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();

    assert_eq!(
            infer_boolean_predicate(&random, &probe, None),
            Some(
                "event_index(43) == 2 and option(162) or event_index(43) == 2 and option(163) or event_index(43) == 3 and option(162) or event_index(43) == 3 and option(163)"
                    .to_string()
            )
        );
    assert_eq!(
        infer_boolean_predicate(&normal, &probe, None),
        Some(
            "event_index(43) == 0 and option(162) or event_index(43) == 0 and option(163)"
                .to_string()
        )
    );
    assert_eq!(
        infer_boolean_predicate(&extended, &probe, None),
        Some(
            "event_index(345) == 11 and option(162) or event_index(345) == 11 and option(163)"
                .to_string()
        )
    );
}

#[test]
fn infers_single_number_lane_color_membership_draw_conditions() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let white = lua
        .load(
            r#"
                return function()
                    local value = main_state.number(450)
                    return value == 1 or value == 3 or value == 5 or value == 7
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();
    let blue = lua
        .load(
            r#"
                return function()
                    local value = main_state.number(450)
                    return value == 2 or value == 4 or value == 6
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();

    assert_eq!(
        infer_boolean_predicate(&white, &probe, None),
        Some(
            "number(450) == 1 or number(450) == 3 or number(450) == 5 or number(450) == 7"
                .to_string()
        )
    );
    assert_eq!(
        infer_boolean_predicate(&blue, &probe, None),
        Some("number(450) == 2 or number(450) == 4 or number(450) == 6".to_string())
    );
}

#[test]
fn infers_loading_or_loaded_before_ready_draw_condition() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).expect("main_state probe");
    lua.globals().set("main_state", main_state).unwrap();
    let function = lua
        .load(
            r#"
                return function()
                    if main_state.option(80) then
                        return true
                    end
                    if not main_state.option(81) then
                        return false
                    end
                    return main_state.timer(40) == main_state.timer_off_value
                end
                "#,
        )
        .eval::<Function>()
        .expect("draw function");

    assert_eq!(
        infer_main_state_two_options_timer_draw_condition(&function, &probe),
        Some("option(80) or option(81) and timer(40) == timer_off".to_string())
    );
}

#[test]
fn infers_keybeam_hold_draw_condition() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let function = lua
            .load(
                r#"
                local off = main_state.timer_off_value
                local last_update_time = off
                local last_key_on_timer = {}
                local last_key_off_timer = {}
                local active = {}
                local fade_start_time = {}
                local suppress_until_key_off = {}
                local lanes = {
                    { display_lane = 1, key_on_timer = 101, key_off_timer = 121, hold_timer = 71 },
                    { display_lane = 2, key_on_timer = 102, key_off_timer = 122, hold_timer = 72 },
                }
                local function update()
                    local now = main_state.time()
                    if now == last_update_time then
                        return
                    end
                    last_update_time = now
                    for _, lane_info in ipairs(lanes) do
                        local lane = lane_info.display_lane
                        local key_on_time = main_state.timer(lane_info.key_on_timer)
                        local key_off_time = main_state.timer(lane_info.key_off_timer)
                        local hold_time = main_state.timer(lane_info.hold_timer)
                        local key_on_changed = key_on_time ~= off and key_on_time ~= last_key_on_timer[lane]
                        local key_off_changed = key_off_time ~= off and key_off_time ~= last_key_off_timer[lane]
                        if key_on_changed then
                            active[lane] = true
                            fade_start_time[lane] = nil
                            suppress_until_key_off[lane] = false
                        end
                        if hold_time ~= off and (active[lane] or key_off_changed) then
                            suppress_until_key_off[lane] = true
                            fade_start_time[lane] = nil
                        end
                        if key_off_changed then
                            active[lane] = true
                            fade_start_time[lane] = key_off_time
                        end
                        last_key_on_timer[lane] = key_on_time
                        last_key_off_timer[lane] = key_off_time
                    end
                end
                return function()
                    update()
                    if not active[1] then
                        return false
                    end
                    if suppress_until_key_off[1] then
                        return false
                    end
                    if fade_start_time[1] ~= nil and main_state.time() >= fade_start_time[1] then
                        return false
                    end
                    return main_state.event_index(501) == 2 or main_state.event_index(501) == 3
                end
                "#,
            )
            .eval::<Function>()
            .unwrap();

    assert_eq!(
            infer_boolean_predicate(&function, &probe, None),
            Some(
                "timer(101) != timer_off and timer(71) == timer_off and event_index(501) == 2 or timer(101) != timer_off and timer(71) == timer_off and event_index(501) == 3"
                    .to_string()
            )
        );
}

#[test]
fn infers_end_of_note_shadow_draw_condition() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let function = lua
        .load(
            r#"
                local TIMER_OFF = main_state.timer_off_value
                local function getRemainNotes()
                    return main_state.number(106)
                        - main_state.number(110)
                        - main_state.number(111)
                        - main_state.number(112)
                        - main_state.number(113)
                        - main_state.number(114)
                end

                return function()
                    if main_state.timer(143) == TIMER_OFF and getRemainNotes() == 0 then
                        return true
                    end
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();

    assert_eq!(
            infer_boolean_predicate(&function, &probe, None),
            Some(
                "timer(143) == timer_off and number(106)-number(110)-number(111)-number(112)-number(113)-number(114) == 0"
                    .to_string()
            )
        );
}

#[test]
fn repairs_keybeam_hold_destination_draws_from_fade_pairs() {
    let mut root = JsonMap::from_iter([(
            "destination".to_string(),
            JsonValue::Array(vec![
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::String("key-beam-thick-pgreat".to_string())),
                    ("draw".to_string(), JsonValue::String("number(0) < 0".to_string())),
                ])),
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::String("key-beam-thick-pgreat".to_string())),
                    ("timer".to_string(), JsonValue::Number(JsonNumber::from(122))),
                    ("loop".to_string(), JsonValue::Number(JsonNumber::from(-1))),
                    ("draw".to_string(), JsonValue::String("event_index(502) == 1".to_string())),
                ])),
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::String("key-beam-thick-great".to_string())),
                    (
                        "draw".to_string(),
                        JsonValue::String(
                            "timer(103) != timer_off and timer(73) == timer_off and event_index(503) == 2"
                                .to_string(),
                        ),
                    ),
                ])),
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::String("key-beam-thick-great".to_string())),
                    ("loop".to_string(), JsonValue::Number(JsonNumber::from(-1))),
                    (
                        "draw".to_string(),
                        JsonValue::String(
                            "event_index(503) == 2 or event_index(503) == 3".to_string(),
                        ),
                    ),
                ])),
            ]),
        )]);

    let mut warnings = vec![
        "skipping unsupported draw function at $.destination[3].draw".to_string(),
        "skipping unsupported field `timer` at $.destination[4]".to_string(),
    ];
    postprocess_lua_skin_json(&mut root, &mut warnings);

    let destinations = root.get("destination").and_then(JsonValue::as_array).unwrap();
    let draw = |index: usize| {
        destinations[index]
            .as_object()
            .and_then(|destination| destination.get("draw"))
            .and_then(JsonValue::as_str)
            .unwrap()
    };
    assert_eq!(draw(0), "keybeam_hold(102) != 0 and event_index(502) == 1");
    assert_eq!(
        draw(2),
        "keybeam_hold(103) != 0 and event_index(503) == 2 or keybeam_hold(103) != 0 and event_index(503) == 3"
    );
    assert_eq!(draw(1), "keybeam_fade(122) != 0 and event_index(502) == 1");
    assert_eq!(
        destinations[3].as_object().and_then(|destination| destination.get("timer")),
        Some(&JsonValue::Number(JsonNumber::from(123)))
    );
    assert!(warnings.is_empty());
}

#[test]
fn infers_keybeam_keyoff_timer_function() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let function = lua
            .load(
                r#"
                local off = main_state.timer_off_value
                local fade_us = 50000
                local last_update_time = off
                local last_key_on_timer = {}
                local last_key_off_timer = {}
                local active = {}
                local fade_start_time = {}
                local lanes = {
                    { display_lane = 1, key_on_timer = 101, key_off_timer = 121, hold_timer = 71 },
                    { display_lane = 2, key_on_timer = 102, key_off_timer = 122, hold_timer = 72 },
                }
                local function update()
                    local now = main_state.time()
                    if now == last_update_time then
                        return
                    end
                    last_update_time = now
                    for _, lane_info in ipairs(lanes) do
                        local lane = lane_info.display_lane
                        local key_on_time = main_state.timer(lane_info.key_on_timer)
                        local key_off_time = main_state.timer(lane_info.key_off_timer)
                        local key_off_changed = key_off_time ~= off and key_off_time ~= last_key_off_timer[lane]
                        if key_on_time ~= off and key_on_time ~= last_key_on_timer[lane] then
                            active[lane] = true
                            fade_start_time[lane] = nil
                        end
                        if key_off_changed then
                            active[lane] = true
                            fade_start_time[lane] = key_off_time
                        end
                        if fade_start_time[lane] and now >= fade_start_time[lane] + fade_us then
                            active[lane] = false
                        end
                        last_key_on_timer[lane] = key_on_time
                        last_key_off_timer[lane] = key_off_time
                    end
                end
                return function()
                    update()
                    local fade_start = fade_start_time[1]
                    if active[1] and fade_start and main_state.time() >= fade_start then
                        return fade_start
                    end
                    return off
                end
                "#,
            )
            .eval::<Function>()
            .unwrap();

    assert_eq!(infer_timer_function_ref(&function, &probe), Some(121));
}

#[test]
fn infers_main_state_judge_as_beatoraja_number_ref() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let value = lua
        .load(
            r#"
                return function()
                    return main_state.judge(1) or 0
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();
    let draw = lua
        .load(
            r#"
                return function()
                    return (main_state.judge(2) or 0) > 0
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();

    assert_eq!(infer_main_state_number_ref(&value, &probe), Some(111));
    assert_eq!(infer_boolean_predicate(&draw, &probe, None), Some("number(112) > 0".to_string()));
}

#[test]
fn infers_weighted_pscore_value_expr_from_judge_counts() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let main_state = create_main_state_stub(&lua, probe.clone()).unwrap();
    lua.globals().set("main_state", main_state).unwrap();
    let function = lua
        .load(
            r#"
                local function clamp(value, min_value, max_value)
                    if value < min_value then
                        return min_value
                    end
                    if value > max_value then
                        return max_value
                    end
                    return value
                end

                return function()
                    local total_notes = main_state.number(74)
                    if not total_notes or total_notes <= 0 then
                        return 0
                    end

                    local cool = main_state.judge(0)
                    local great = main_state.judge(1)
                    local good = main_state.judge(2)
                    local raw = 100000 * ((cool * 1.0) + (great * 0.7) + (good * 0.4)) / total_notes
                    return clamp(math.floor(raw), 0, 100000)
                end
                "#,
        )
        .eval::<Function>()
        .unwrap();

    assert_eq!(
        infer_value_float_expr(&function, &probe),
        Some(
            "floor((100000*number(110)+70000*number(111)+40000*number(112))/number(74))"
                .to_string()
        )
    );
}

#[test]
fn infers_peaceful_play_gauge_value_builtins() {
    let lua = Lua::new();
    let probe = Arc::new(Mutex::new(MainStateProbe::default()));
    let function = lua.load("return function() return 0 end").eval::<Function>().unwrap();

    for (id, expected) in [
        ("val-gauge-percent-integer", SKIN_EXPR_GAUGE_PERCENT_INTEGER),
        ("val-gauge-percent-fraction", SKIN_EXPR_GAUGE_PERCENT_FRACTION),
        ("val-gauge-amount-integer", SKIN_EXPR_GAUGE_AMOUNT_INTEGER),
        ("val-gauge-amount-fraction", SKIN_EXPR_GAUGE_AMOUNT_FRACTION),
    ] {
        assert_eq!(
            infer_bmz_builtin_value_expr(&function, Some(id), &probe),
            Some(expected.to_string())
        );
    }
}

fn unique_skin_test_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bmz-lua-{tag}-{nanos}-{n}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn beatoraja_skin_alias_accepts_renamed_skin_root() {
    let root = unique_skin_test_dir("renamed-root").join("mz-select");
    fs::create_dir_all(root.join("customize/advanced")).unwrap();
    fs::write(root.join("customize/advanced/enable.txt"), "parts.lua\n").unwrap();

    let resolved =
        resolve_skin_io_path(&root, "skin/m_select/customize/advanced/enable.txt").unwrap();

    assert_eq!(
        resolved,
        canonicalize_skin_path(&root.join("customize/advanced/enable.txt")).unwrap()
    );
}

#[test]
fn default_skin_file_uses_random_sentinel_for_random_def() {
    let root = unique_skin_test_dir("random-def");
    fs::create_dir_all(root.join("bg")).unwrap();
    fs::write(root.join("bg/one.mp4"), []).unwrap();
    fs::write(root.join("bg/two.mp4"), []).unwrap();
    let filepath: JsonValue =
        serde_json::from_str(r#"{ "name": "BG", "path": "bg/*.mp4", "def": "Random" }"#).unwrap();

    assert_eq!(
        default_skin_file_from_filepath(&root, "bg/*.mp4", &filepath).as_deref(),
        Some(RANDOM_FILE_SELECTION)
    );
}

#[test]
fn default_skin_file_returns_beatoraja_filename_selection() {
    let root = unique_skin_test_dir("filename-default");
    fs::create_dir_all(root.join("bg")).unwrap();
    fs::write(root.join("bg/one.mp4"), []).unwrap();
    fs::write(root.join("bg/two.mp4"), []).unwrap();
    let filepath: JsonValue =
        serde_json::from_str(r#"{ "name": "BG", "path": "bg/*.mp4", "def": "two" }"#).unwrap();

    assert_eq!(
        default_skin_file_from_filepath(&root, "bg/*.mp4", &filepath).as_deref(),
        Some("two.mp4")
    );
}

#[test]
fn default_skin_file_prefers_default_stem_when_def_missing() {
    let root = unique_skin_test_dir("default-stem");
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/pastel.png"), []).unwrap();
    fs::write(root.join("notes/default.png"), []).unwrap();
    let filepath: JsonValue =
        serde_json::from_str(r#"{ "name": "Note", "path": "notes/*.png" }"#).unwrap();

    assert_eq!(
        default_skin_file_from_filepath(&root, "notes/*.png", &filepath).as_deref(),
        Some("default.png")
    );
}

#[test]
fn property_default_matches_item_name_not_numeric_op_string() {
    let property: JsonValue = serde_json::from_str(
        r#"
            {
                "name": "Graph",
                "def": "923",
                "item": [
                    { "name": "AC", "op": 922 },
                    { "name": "TYPE-M", "op": 923 }
                ]
            }
            "#,
    )
    .unwrap();
    let items = property.get("item").and_then(JsonValue::as_array).unwrap();

    assert_eq!(default_property_op(&property, items), Some(922));
}

#[test]
fn selected_numeric_option_must_exist_in_items() {
    let items: Vec<JsonValue> = serde_json::from_str(
        r#"
            [
                { "name": "AC", "op": 922 },
                { "name": "TYPE-M", "op": 923 }
            ]
            "#,
    )
    .unwrap();

    assert_eq!(option_value_to_op(&items, "923"), Some(923));
    assert_eq!(option_value_to_op(&items, "999"), None);
}

#[test]
fn property_options_accept_integral_lua_numbers() {
    let property: JsonValue = serde_json::from_str(
        r#"
            {
                "name": "Key Beam Length",
                "def": "100%",
                "item": [
                    { "name": "100%", "op": 11400.0 },
                    { "name": "90%", "op": 11401.0 }
                ]
            }
            "#,
    )
    .unwrap();
    let header = serde_json::json!({ "property": [property] });
    let mut warnings = Vec::new();

    let options = skin_config_options_from_header(
        &header,
        &BTreeMap::from([("Key Beam Length".to_string(), "90%".to_string())]),
        &mut warnings,
    );

    assert_eq!(options.get("Key Beam Length"), Some(&11401));
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn property_options_reject_fractional_lua_numbers() {
    let items = vec![serde_json::json!({ "name": "invalid", "op": 11400.5 })];

    assert_eq!(option_value_to_op(&items, "invalid"), None);
}

#[test]
fn get_path_accepts_beatoraja_filename_selection() {
    let root = unique_skin_test_dir("filename-getpath");
    fs::create_dir_all(root.join("bg")).unwrap();
    fs::write(root.join("bg/one.mp4"), []).unwrap();
    let skin_files = BTreeMap::from([("bg/*.mp4".to_string(), "one.mp4".to_string())]);

    let resolved = skin_config_get_path(&root, "bg/*.mp4", &skin_files).unwrap();

    assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("one.mp4"));
}

#[test]
fn get_path_randomizes_when_selection_is_random_sentinel() {
    let root = unique_skin_test_dir("random-getpath");
    fs::create_dir_all(root.join("bg")).unwrap();
    fs::write(root.join("bg/one.mp4"), []).unwrap();
    fs::write(root.join("bg/two.mp4"), []).unwrap();
    let skin_files = BTreeMap::from([("bg/*.mp4".to_string(), RANDOM_FILE_SELECTION.to_string())]);

    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        let resolved = skin_config_get_path(&root, "bg/*.mp4", &skin_files).unwrap();
        let name =
            resolved.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string();
        assert!(name == "one.mp4" || name == "two.mp4", "unexpected match {name}");
        seen.insert(name);
    }
    assert_eq!(seen.len(), 2, "Random selection should pick randomly among matches");
}

#[test]
fn repairs_strictly_recognized_malformed_destination_ops() {
    let mut value = serde_json::json!({
        "type": 7,
        "destination": [
            {
                "id": "rankBig_AAA",
                "op": {
                    "1": 300,
                    "2": 920,
                    "loop": 100,
                    "filter": 1,
                    "dst": [{"x": 77, "y": 800, "w": 400, "h": 510}]
                }
            },
            {
                "id": "AAA_BG",
                "op": [90, [90, 300]],
                "dst": [{"x": 0, "y": 0, "w": 1, "h": 1}]
            }
        ]
    });
    let mut warnings =
        vec!["mixed lua table converted to object at $.destination[1].op".to_string()];

    postprocess_lua_skin_json(value.as_object_mut().unwrap(), &mut warnings);

    assert_eq!(value["destination"][0]["op"], serde_json::json!([300, 920]));
    assert_eq!(value["destination"][0]["loop"], 100);
    assert_eq!(value["destination"][0]["filter"], 1);
    assert!(value["destination"][0]["dst"].is_array());
    assert_eq!(value["destination"][1]["op"], serde_json::json!([90, 300]));
    assert_eq!(warnings, ["repaired 2 malformed destination op tables"]);

    let document: bmz_skin_document::SkinDocument =
        serde_json::from_value(value.clone()).expect("repaired destinations should decode");
    let destinations = document
        .destination
        .iter()
        .filter_map(|entry| match entry {
            bmz_skin_document::DestinationListEntry::Single(destination) => Some(destination),
            bmz_skin_document::DestinationListEntry::Conditional { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(destinations[0].op, [300, 920]);
    assert_eq!(destinations[1].op, [90, 300]);

    let once = value.clone();
    let warning_count = warnings.len();
    postprocess_lua_skin_json(value.as_object_mut().unwrap(), &mut warnings);
    assert_eq!(value, once);
    assert_eq!(warnings.len(), warning_count);
}

#[test]
fn leaves_ambiguous_destination_ops_unmodified() {
    let mut value = serde_json::json!({
        "destination": [
            {"id": "sparse", "op": {"1": 90, "3": 300, "dst": []}},
            {"id": "unknown", "op": {"1": 90, "custom": 1, "dst": []}},
            {"id": "conflict", "loop": 200, "op": {"1": 90, "loop": 100, "dst": []}},
            {"id": "different-prefix", "op": [90, [300]], "dst": []},
            {"id": "deep", "op": [90, [90, [300]]], "dst": []}
        ]
    });
    let original = value.clone();
    let mut warnings = Vec::new();

    postprocess_lua_skin_json(value.as_object_mut().unwrap(), &mut warnings);

    assert_eq!(value, original);
    assert!(warnings.is_empty());
}
