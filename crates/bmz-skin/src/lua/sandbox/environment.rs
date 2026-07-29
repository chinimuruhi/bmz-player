use super::*;

pub(super) fn install_sandbox(
    lua: &Lua,
    root: &Path,
    options: &BTreeMap<String, String>,
    skin_config_options: Option<&BTreeMap<String, i64>>,
    skin_files: &BTreeMap<String, String>,
    skin_file_dependency_names: &BTreeMap<String, String>,
    skin_offsets: &BTreeMap<String, LuaSkinOffsetValue>,
    runtime_state: &LuaLoadRuntimeState,
    virtual_io_files: &BTreeMap<String, String>,
    load_dependencies: Option<Arc<Mutex<SkinLoadDependencies>>>,
) -> Result<Arc<Mutex<MainStateProbe>>> {
    lua.set_memory_limit(LUA_MEMORY_LIMIT_BYTES).context("failed to set Lua skin memory limit")?;
    let main_state_probe = Arc::new(Mutex::new(MainStateProbe::default()));
    if let Some(load_dependencies) = load_dependencies.clone() {
        let mut probe =
            main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
        probe.load_dependencies = Some(load_dependencies);
    }
    if !runtime_state.number_values.is_empty() {
        let mut probe =
            main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
        probe.number_values = runtime_state.number_values.clone();
    }
    if !runtime_state.text_values.is_empty() {
        let mut probe =
            main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
        probe.text_values = runtime_state.text_values.clone();
    }
    if !runtime_state.option_values.is_empty() {
        let mut probe =
            main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
        probe.option_values = runtime_state.option_values.clone();
    }
    if !runtime_state.event_index_values.is_empty() {
        let mut probe =
            main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
        probe.event_index_values = runtime_state.event_index_values.clone();
    }
    if !runtime_state.offset_id_values.is_empty() {
        let mut probe =
            main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
        probe.offset_values = runtime_state.offset_id_values.clone();
    }
    let globals = lua.globals();
    if let Some(skin_config_options) = skin_config_options {
        let skin_config = lua.create_table()?;
        let option =
            create_skin_config_option_table(lua, skin_config_options, load_dependencies.clone())?;
        skin_config.set("option", option)?;
        let offset = lua.create_table()?;
        for (name, value) in skin_offsets {
            let offset_value = lua.create_table()?;
            offset_value.set("x", value.x)?;
            offset_value.set("y", value.y)?;
            offset_value.set("w", value.w)?;
            offset_value.set("h", value.h)?;
            offset_value.set("r", value.r)?;
            offset_value.set("a", value.a)?;
            offset.set(name.as_str(), offset_value)?;
        }
        if let Some(load_dependencies) = &load_dependencies
            && let Ok(mut dependencies) = load_dependencies.lock()
        {
            dependencies.offset_values.extend(skin_offsets.clone());
        }
        skin_config.set("offset", offset)?;
        let root_for_get_path = root.to_path_buf();
        let skin_files_for_get_path = skin_files.clone();
        let skin_file_dependency_names_for_get_path = skin_file_dependency_names.clone();
        let dependencies_for_get_path = load_dependencies.clone();
        let get_path = lua.create_function(move |_, requested: String| {
            record_skin_config_file_dependency(
                &requested,
                &skin_file_dependency_names_for_get_path,
                dependencies_for_get_path.as_ref(),
            );
            skin_config_get_path(&root_for_get_path, &requested, &skin_files_for_get_path)
                .map(|path| path.to_string_lossy().to_string())
                .map_err(mlua::Error::external)
        })?;
        skin_config.set("get_path", get_path)?;
        globals.set("skin_config", skin_config)?;
    }
    globals.set("os", create_os_stub(lua, main_state_probe.clone())?)?;
    globals.set("io", create_io_stub(lua, root, virtual_io_files, load_dependencies.clone())?)?;
    globals.set("debug", Value::Nil)?;
    if let Ok(package) = globals.get::<Table>("package") {
        package.set("loadlib", Value::Nil)?;
    }

    let print = lua.create_function(|_, args: Variadic<Value>| {
        let parts =
            args.into_iter().map(|value| lua_value_to_log_string(&value)).collect::<Vec<_>>();
        eprintln!("lua: {}", parts.join("\t"));
        Ok(())
    })?;
    globals.set("print", print)?;

    let option_table = lua.create_table()?;
    for (key, value) in options {
        option_table.set(key.as_str(), value.as_str())?;
    }
    let bmz = lua.create_table()?;
    bmz.set("option", option_table.clone())?;
    let options_for_getter = options.clone();
    let get_option = lua.create_function(move |_, (name, default): (String, Option<String>)| {
        Ok(options_for_getter.get(&name).cloned().or(default).unwrap_or_default())
    })?;
    bmz.set("get_option", get_option)?;
    globals.set("bmz", bmz)?;

    let sandbox_root = root.to_path_buf();
    let root_for_dofile = sandbox_root.clone();
    let dependencies_for_dofile = load_dependencies.clone();
    let dofile = lua.create_function(move |lua, path: String| {
        let path =
            resolve_lua_path(&root_for_dofile, &path, false).map_err(mlua::Error::external)?;
        record_lua_loaded_file_dependency(&path, dependencies_for_dofile.as_ref());
        let source = fs::read_to_string(&path).map_err(mlua::Error::external)?;
        lua.load(&source).set_name(path.to_string_lossy().as_ref()).eval::<Value>()
    })?;
    globals.set("dofile", dofile)?;

    let root_for_loadfile = sandbox_root.clone();
    let dependencies_for_loadfile = load_dependencies.clone();
    let loadfile = lua.create_function(move |lua, path: String| {
        let path =
            resolve_lua_path(&root_for_loadfile, &path, false).map_err(mlua::Error::external)?;
        record_lua_loaded_file_dependency(&path, dependencies_for_loadfile.as_ref());
        let source = fs::read_to_string(&path).map_err(mlua::Error::external)?;
        lua.load(&source).set_name(path.to_string_lossy().as_ref()).into_function()
    })?;
    globals.set("loadfile", loadfile)?;

    let main_state = create_main_state_stub(lua, main_state_probe.clone())?;
    lua.globals().set("bmz_main_state", main_state)?;

    let root = sandbox_root;
    let probe_for_require = main_state_probe.clone();
    let dependencies_for_require = load_dependencies.clone();
    let require = lua.create_function(move |lua, module: String| {
        if module == "main_state" {
            return lua.globals().get("bmz_main_state");
        }
        if module == "timer_util" {
            return create_timer_util_module(lua, probe_for_require.clone());
        }
        if module == "event_util" {
            return create_event_util_module(lua);
        }
        if module == "luajava" {
            return create_luajava_stub(lua);
        }
        let globals = lua.globals();
        let package: Table = globals.get("package")?;
        let loaded: Table = package.get("loaded")?;
        if let Ok(cached) = loaded.get::<Value>(module.as_str())
            && !matches!(cached, Value::Nil)
        {
            return Ok(cached);
        }

        let path = resolve_lua_path(&root, &module, true).map_err(mlua::Error::external)?;
        record_lua_loaded_file_dependency(&path, dependencies_for_require.as_ref());
        let source = fs::read_to_string(&path).map_err(mlua::Error::external)?;
        let value = lua.load(&source).set_name(path.to_string_lossy().as_ref()).eval::<Value>()?;
        let value = if matches!(value, Value::Nil) { Value::Boolean(true) } else { value };
        loaded.set(module, value.clone())?;
        Ok(value)
    })?;
    globals.set("require", require)?;

    let timer_fn_map = lua.create_table()?;
    let timer_fn_metatable = lua.create_table()?;
    timer_fn_metatable.set("__mode", "k")?;
    timer_fn_map.set_metatable(Some(timer_fn_metatable));
    globals.set("bmz_timer_fn_map", timer_fn_map)?;

    Ok(main_state_probe)
}
