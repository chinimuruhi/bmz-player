struct ExecutedLuaSkin {
    value: JsonValue,
    warnings: Vec<String>,
    files: BTreeMap<String, String>,
    dependencies: SkinLoadDependencies,
    lua_runtime: Option<LuaSkinRuntime>,
    runtime_draw_paths: Vec<String>,
}

fn execute_lua_skin(
    input: &Path,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
    virtual_io_files: &BTreeMap<String, String>,
) -> Result<ExecutedLuaSkin> {
    let input = canonicalize_skin_path(input)
        .with_context(|| format!("failed to canonicalize input: {}", input.display()))?;
    let parent =
        input.parent().ok_or_else(|| anyhow!("input path has no parent: {}", input.display()))?;
    let root = canonicalize_skin_path(parent)
        .with_context(|| format!("failed to canonicalize skin root: {}", input.display()))?;

    let mut warnings = Vec::new();
    let mut table_budget = TableBudget::default();
    let source = fs::read_to_string(&input)
        .with_context(|| format!("failed to read lua skin: {}", input.display()))?;

    let header_lua = Lua::new();
    let header_instruction_budget = install_instruction_limit(&header_lua);
    // The header pass intentionally uses neutral main_state values, but it must
    // see the same read-only virtual filesystem as the document pass. Some
    // skins read compatibility configuration while their required modules are
    // initialized, before deciding whether to return a header or a document.
    let header_probe = install_sandbox(
        &header_lua,
        &root,
        options,
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        virtual_io_files,
        None,
    )?;
    let header = header_lua
        .load(&source)
        .set_name(input.to_string_lossy().as_ref())
        .eval::<Value>()
        .with_context(|| format!("failed to execute lua skin header: {}", input.display()))?;
    header_instruction_budget.begin_inference();
    let header_json = lua_value_to_json(
        &header_lua,
        header,
        "$",
        0,
        &mut warnings,
        &header_probe,
        &header_instruction_budget,
        &mut table_budget,
    )?;
    let skin_options = skin_config_options_from_header(&header_json, options, &mut warnings);
    let skin_files = skin_files_from_header(&root, &header_json, files);
    let skin_named_files = skin_named_files_from_header(&root, &header_json, files);
    let skin_offsets = skin_config_offsets_from_header(&header_json, runtime_state);
    let mut resolved_runtime_state = runtime_state.clone();
    resolved_runtime_state.offset_id_values.clear();
    for (name, id) in skin_offset_definitions_from_header(&header_json) {
        resolved_runtime_state
            .offset_id_values
            .insert(id, lua_skin_offset_value(runtime_state, &name, Some(id)));
    }
    // ヘッダ pass では skin_config / 全 option が未注入のため draw/value 推論が失敗しうる。
    // 本 pass の警告だけ残す。
    warnings.retain(|warning| {
        !warning.starts_with("skipping unsupported draw function at ")
            && !warning.starts_with("skipping unsupported value function at ")
            && !warning.starts_with("skipping unsupported custom timer function ")
            && !warning.starts_with("mixed lua table converted to object at ")
    });

    // Lua スキンには、無効な `op` を持つ destination でも Lua の table 構築時に
    // 座標を評価するものがある。選択中の property ではその座標が初期化されない場合、
    // 最終的には描画されない destination でも nil 算術でロード全体が失敗してしまう。
    // その場合だけ各 property の末尾選択肢で再評価する。描画時の有効 op は呼び出し側が
    // 元の選択値から設定するため、この再試行で無効 destination が表示されることはない。
    let fallback_skin_options = fallback_skin_config_options(&header_json, &skin_options);
    let mut use_fallback_options = false;
    let (mut json, dependencies, main_state_probe, result_panel_default, scene_audio_actions) = loop {
        let active_skin_options =
            if use_fallback_options { &fallback_skin_options } else { &skin_options };
        let lua = Lua::new();
        let instruction_budget = install_instruction_limit(&lua);
        let dependencies = Arc::new(Mutex::new(SkinLoadDependencies::default()));
        let main_state_probe = install_sandbox(
            &lua,
            &root,
            options,
            Some(active_skin_options),
            &skin_files,
            &skin_file_dependency_names_from_header(&header_json),
            &skin_offsets,
            &resolved_runtime_state,
            virtual_io_files,
            Some(dependencies.clone()),
        )?;
        let value = match lua
            .load(&source)
            .set_name(input.to_string_lossy().as_ref())
            .eval::<Value>()
        {
            Ok(value) => value,
            Err(error)
                if !use_fallback_options
                    && fallback_skin_options != skin_options
                    && lua_nil_arithmetic_error(&error) =>
            {
                use_fallback_options = true;
                warnings.push(
                    "retried lua skin with fallback property options after nil arithmetic in an inactive destination"
                        .to_string(),
                );
                continue;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to execute lua skin: {}", input.display()));
            }
        };
        let scene_audio_actions = {
            let mut probe =
                main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
            let actions = probe.take_audio_actions();
            probe.capture_audio_actions = false;
            actions
        };
        instruction_budget.begin_inference();
        let json = lua_value_to_json(
            &lua,
            value,
            "$",
            0,
            &mut warnings,
            &main_state_probe,
            &instruction_budget,
            &mut table_budget,
        )?;
        let result_panel_default =
            lua.globals().get::<Value>("Expand_op").ok().and_then(lua_result_panel_value).or_else(
                || main_state_probe.lock().ok().and_then(|probe| probe.result_panel_default),
            );
        break (json, dependencies, main_state_probe, result_panel_default, scene_audio_actions);
    };
    record_static_skin_config_option_dependencies(&source, &skin_options, &dependencies);
    if use_fallback_options && let Ok(mut dependencies) = dependencies.lock() {
        // fallback 側の Lua 分岐で収集した依存関係は元選択の cache key に使えない。
        dependencies.opaque = true;
    }

    let skin_audio_root = root.clone();
    if let JsonValue::Object(ref mut root) = json {
        postprocess_lua_skin_json(root, &mut warnings);

        if let Some(panel) = result_panel_default {
            root.insert(
                "resultPanelDefault".to_string(),
                JsonValue::Number(JsonNumber::from(panel)),
            );
        }

        let timers = main_state_probe
            .lock()
            .ok()
            .map(|probe| probe.dynamic_timers.clone())
            .unwrap_or_default();
        if !timers.is_empty() {
            let entries = timers.into_iter().map(|(id, observe)| {
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::Number(JsonNumber::from(id))),
                    ("observe".to_string(), JsonValue::String(observe)),
                ]))
            });
            root.insert("dynamicTimer".to_string(), JsonValue::Array(entries.collect()));
        }
        let fixed_delay_timers = main_state_probe
            .lock()
            .ok()
            .map(|probe| probe.fixed_delay_timers.clone())
            .unwrap_or_default();
        if !fixed_delay_timers.is_empty() {
            let entries = fixed_delay_timers.into_iter().map(|(id, source_timer, delay_ms)| {
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::Number(JsonNumber::from(id))),
                    ("sourceTimer".to_string(), JsonValue::Number(JsonNumber::from(source_timer))),
                    ("delayMs".to_string(), JsonValue::Number(JsonNumber::from(delay_ms))),
                ]))
            });
            root.insert("fixedDelayTimer".to_string(), JsonValue::Array(entries.collect()));
        }
        let runtime_flags = main_state_probe
            .lock()
            .ok()
            .map(|probe| probe.runtime_flags.clone())
            .unwrap_or_default();
        if !runtime_flags.is_empty() {
            let entries = runtime_flags.into_iter().map(|flag| {
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::Number(JsonNumber::from(flag.id))),
                    ("initial".to_string(), JsonValue::Bool(flag.initial)),
                ]))
            });
            root.insert("runtimeFlag".to_string(), JsonValue::Array(entries.collect()));
        }
        let runtime_events = main_state_probe
            .lock()
            .ok()
            .map(|probe| probe.runtime_events.clone())
            .unwrap_or_default();
        if !runtime_events.is_empty() {
            let entries = runtime_events.into_iter().map(|(id, toggle_flags)| {
                JsonValue::Object(JsonMap::from_iter([
                    ("id".to_string(), JsonValue::Number(JsonNumber::from(id))),
                    (
                        "toggleFlags".to_string(),
                        JsonValue::Array(
                            toggle_flags
                                .into_iter()
                                .map(|flag_id| JsonValue::Number(JsonNumber::from(flag_id)))
                                .collect(),
                        ),
                    ),
                ]))
            });
            root.insert("runtimeEvent".to_string(), JsonValue::Array(entries.collect()));
        }
        if !scene_audio_actions.is_empty() {
            root.insert(
                "sceneAudio".to_string(),
                JsonValue::Array(
                    scene_audio_actions.into_iter().map(lua_audio_action_to_json).collect(),
                ),
            );
        }
        normalize_lua_skin_audio_paths(&skin_audio_root, root, &mut warnings);
    }

    let unsupported_dynamic_timers = main_state_probe
        .lock()
        .ok()
        .map(|probe| probe.unsupported_dynamic_timers.clone())
        .unwrap_or_default();
    warnings.extend(unsupported_dynamic_timers.into_iter().map(|id| {
        format!(
            "timer_util.timer_observe_boolean callback for generated timer {id} could not be inferred; timer remains inactive"
        )
    }));
    let load_time_constant_dynamic_timers = main_state_probe
        .lock()
        .ok()
        .map(|probe| probe.load_time_constant_dynamic_timers.clone())
        .unwrap_or_default();
    warnings.extend(load_time_constant_dynamic_timers.into_iter().map(|id| {
        format!(
            "timer_util.timer_observe_boolean callback for generated timer {id} was fixed to its load-time value; runtime Lua state changes are unsupported"
        )
    }));

    let runtime_draw_paths = main_state_probe
        .lock()
        .map_err(|_| anyhow!("main_state probe lock poisoned"))?
        .runtime_draw_paths
        .clone();
    let active_skin_options =
        if use_fallback_options { &fallback_skin_options } else { &skin_options };
    let lua_runtime = if runtime_draw_paths.is_empty() {
        None
    } else {
        match build_lua_skin_runtime(
            &input,
            &root,
            &source,
            options,
            active_skin_options,
            &skin_files,
            &skin_file_dependency_names_from_header(&header_json),
            &skin_offsets,
            runtime_state,
            virtual_io_files,
            &runtime_draw_paths,
        ) {
            Ok(runtime) => Some(runtime),
            Err(error) => {
                warnings.push(format!(
                    "ERROR: failed to build Lua draw callback runtime for {}: {error:#}; callbacks fall back to false",
                    input.display()
                ));
                None
            }
        }
    };
    let mut dependencies =
        dependencies.lock().map_err(|_| anyhow!("lua dependency tracker lock poisoned"))?.clone();
    // A cached document cannot safely clone its sidecar VM. Keeping runtime
    // callback documents out of the document cache preserves registry/VM IDs.
    dependencies.opaque |= !runtime_draw_paths.is_empty();
    Ok(ExecutedLuaSkin {
        value: json,
        warnings,
        files: skin_named_files,
        dependencies,
        lua_runtime,
        runtime_draw_paths,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_lua_skin_runtime(
    input: &Path,
    root: &Path,
    source: &str,
    options: &BTreeMap<String, String>,
    skin_config_options: &BTreeMap<String, i64>,
    skin_files: &BTreeMap<String, String>,
    skin_file_dependency_names: &BTreeMap<String, String>,
    skin_offsets: &BTreeMap<String, LuaSkinOffsetValue>,
    runtime_state: &LuaLoadRuntimeState,
    virtual_io_files: &BTreeMap<String, String>,
    runtime_draw_paths: &[String],
) -> Result<LuaSkinRuntime> {
    let lua = Lua::new();
    let instruction_budget = install_instruction_limit(&lua);
    // This is a clean runtime VM. Installing the sandbox and evaluating the
    // skin creates closures, but no draw callback is invoked here.
    install_sandbox(
        &lua,
        root,
        options,
        Some(skin_config_options),
        skin_files,
        skin_file_dependency_names,
        skin_offsets,
        runtime_state,
        virtual_io_files,
        None,
    )?;
    let value = lua
        .load(source)
        .set_name(input.to_string_lossy().as_ref())
        .eval::<Value>()
        .with_context(|| format!("failed to execute runtime Lua skin: {}", input.display()))?;

    let mut callbacks = Vec::with_capacity(runtime_draw_paths.len());
    for (callback_id, path) in runtime_draw_paths.iter().enumerate() {
        let callback =
            lua_value_at_field_path(value.clone(), path).ok().and_then(|value| match value {
                Value::Function(function) => lua.create_registry_value(function).ok(),
                _ => None,
            });
        if callback.is_some() {
            tracing::debug!(
                skin = %input.display(),
                callback_id,
                field_path = path,
                classification = "RUNTIME",
                "registered Lua draw callback"
            );
        } else {
            tracing::warn!(
                skin = %input.display(),
                callback_id,
                field_path = path,
                classification = "ERROR",
                "failed to register Lua draw callback; callback falls back to false"
            );
        }
        callbacks.push(LuaRuntimeCallback { path: path.clone(), key: callback });
    }
    let main_state: Table = lua.globals().get("bmz_main_state")?;
    let main_state_key = lua.create_registry_value(main_state)?;
    Ok(LuaSkinRuntime {
        lua,
        callbacks,
        main_state_key,
        instruction_budget,
        skin_path: input.to_path_buf(),
        failed_callbacks: BTreeSet::new(),
        failure_log_count: 0,
    })
}

fn lua_value_at_field_path(mut value: Value, path: &str) -> Result<Value> {
    let bytes = path.as_bytes();
    if bytes.first() != Some(&b'$') {
        bail!("invalid Lua callback field path: {path}");
    }
    let mut cursor = 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'.' => {
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && !matches!(bytes[cursor], b'.' | b'[') {
                    cursor += 1;
                }
                if start == cursor {
                    bail!("empty Lua callback field in path: {path}");
                }
                let key = &path[start..cursor];
                let Value::Table(table) = value else {
                    bail!("Lua callback path parent is not a table at {key}: {path}");
                };
                value = table.get::<Value>(key)?;
            }
            b'[' => {
                cursor += 1;
                let start = cursor;
                while cursor < bytes.len() && bytes[cursor] != b']' {
                    cursor += 1;
                }
                if cursor >= bytes.len() {
                    bail!("unterminated Lua callback array index: {path}");
                }
                let index = path[start..cursor]
                    .parse::<i64>()
                    .with_context(|| format!("invalid Lua callback array index: {path}"))?;
                cursor += 1;
                let Value::Table(table) = value else {
                    bail!("Lua callback array parent is not a table at [{index}]: {path}");
                };
                value = table.get::<Value>(index)?;
            }
            _ => bail!("invalid Lua callback field path: {path}"),
        }
    }
    Ok(value)
}
