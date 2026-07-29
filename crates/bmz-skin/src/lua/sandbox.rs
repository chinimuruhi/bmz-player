use super::*;

#[derive(Clone)]
pub(super) struct LuaInstructionBudget {
    total_remaining: Arc<AtomicI64>,
    callback_remaining: Arc<AtomicI64>,
}

impl LuaInstructionBudget {
    pub(super) fn begin_inference(&self) {
        self.total_remaining.store(LUA_INFERENCE_INSTRUCTION_LIMIT, Ordering::Relaxed);
        self.begin_callback();
    }

    pub(super) fn begin_callback(&self) {
        self.callback_remaining.store(LUA_INSTRUCTION_LIMIT, Ordering::Relaxed);
    }

    pub(super) fn begin_runtime_callback(&self) {
        // Runtime has no inference-wide work to preserve. Reset both counters so
        // every draw invocation gets an independent, bounded instruction slice.
        self.total_remaining.store(LUA_INSTRUCTION_LIMIT, Ordering::Relaxed);
        self.begin_callback();
    }
}

pub(super) fn install_instruction_limit(lua: &Lua) -> LuaInstructionBudget {
    let budget = LuaInstructionBudget {
        total_remaining: Arc::new(AtomicI64::new(LUA_INSTRUCTION_LIMIT)),
        callback_remaining: Arc::new(AtomicI64::new(LUA_INSTRUCTION_LIMIT)),
    };
    let total_remaining = budget.total_remaining.clone();
    let callback_remaining = budget.callback_remaining.clone();
    lua.set_hook(HookTriggers::new().every_nth_instruction(LUA_HOOK_INTERVAL), move |_, _| {
        let interval = i64::from(LUA_HOOK_INTERVAL);
        if total_remaining.fetch_sub(interval, Ordering::Relaxed) <= interval
            || callback_remaining.fetch_sub(interval, Ordering::Relaxed) <= interval
        {
            Err(mlua::Error::runtime("lua skin instruction limit exceeded"))
        } else {
            Ok(VmState::Continue)
        }
    });
    budget
}

#[derive(Debug)]
pub(super) struct TableBudget {
    remaining_entries: usize,
}

impl Default for TableBudget {
    fn default() -> Self {
        Self { remaining_entries: LUA_MAX_TABLE_ENTRIES }
    }
}

impl TableBudget {
    pub(super) fn consume(&mut self, count: usize, path: &str) -> Result<()> {
        if count > self.remaining_entries {
            bail!("lua table entry limit exceeded at {path}");
        }
        self.remaining_entries -= count;
        Ok(())
    }
}

pub(super) fn create_skin_config_option_table(
    lua: &Lua,
    skin_config_options: &BTreeMap<String, i64>,
    load_dependencies: Option<Arc<Mutex<SkinLoadDependencies>>>,
) -> Result<Table> {
    let option = lua.create_table()?;
    let option_values = skin_config_options.clone();
    let dependencies_for_index = load_dependencies.clone();
    let index = lua.create_function(move |_, (_table, key): (Table, Value)| {
        let Value::String(key) = key else {
            return Ok(Value::Nil);
        };
        let key = key.to_str()?;
        let Some(value) = option_values.get(key.as_ref()) else {
            return Ok(Value::Nil);
        };
        if let Ok(option_id) = i32::try_from(*value) {
            record_load_dependency_option(dependencies_for_index.as_ref(), option_id, true);
        }
        Ok(Value::Integer(*value))
    })?;
    let option_values_for_pairs = skin_config_options.clone();
    let dependencies_for_pairs = load_dependencies;
    let pairs = lua.create_function(move |lua, _: Table| {
        let pairs_table = lua.create_table()?;
        for (key, value) in &option_values_for_pairs {
            pairs_table.set(key.as_str(), *value)?;
            if let Ok(option_id) = i32::try_from(*value) {
                record_load_dependency_option(dependencies_for_pairs.as_ref(), option_id, true);
            }
        }
        let next = lua.globals().get::<Function>("next")?;
        Ok((next, pairs_table, Value::Nil))
    })?;
    let metatable = lua.create_table()?;
    metatable.set("__index", index)?;
    metatable.set("__pairs", pairs)?;
    option.set_metatable(Some(metatable));
    Ok(option)
}

pub(super) fn record_load_dependency_option(
    dependencies: Option<&Arc<Mutex<SkinLoadDependencies>>>,
    option_id: i32,
    value: bool,
) {
    if let Some(dependencies) = dependencies
        && let Ok(mut dependencies) = dependencies.lock()
    {
        dependencies.option_values.insert(option_id, value);
    }
}

pub(super) fn record_skin_config_file_dependency(
    requested: &str,
    skin_file_dependency_names: &BTreeMap<String, String>,
    dependencies: Option<&Arc<Mutex<SkinLoadDependencies>>>,
) {
    let requested = requested.replace('\\', "/");
    let Some(name) = skin_config_file_dependency_name(&requested, skin_file_dependency_names)
    else {
        return;
    };
    if let Some(dependencies) = dependencies
        && let Ok(mut dependencies) = dependencies.lock()
    {
        dependencies.files.insert(name);
    }
}

pub(super) fn skin_config_file_dependency_name(
    requested: &str,
    skin_file_dependency_names: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(name) = skin_file_dependency_names.get(requested) {
        return Some(name.clone());
    }
    let (requested_prefix, _) = requested.split_once('*')?;
    skin_file_dependency_names.iter().find_map(|(configured, name)| {
        let (configured_prefix, _) = configured.split_once('*')?;
        (requested_prefix == configured_prefix).then(|| name.clone())
    })
}

pub(super) fn record_lua_loaded_file_dependency(
    path: &Path,
    dependencies: Option<&Arc<Mutex<SkinLoadDependencies>>>,
) {
    let Some(dependencies) = dependencies else {
        return;
    };
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dependency =
        SkinLoadedFileDependency { modified: metadata.modified().ok(), len: metadata.len() };
    if let Ok(mut dependencies) = dependencies.lock() {
        dependencies.loaded_files.insert(path, dependency);
    }
}

pub(super) fn record_static_skin_config_option_dependencies(
    source: &str,
    skin_config_options: &BTreeMap<String, i64>,
    dependencies: &Arc<Mutex<SkinLoadDependencies>>,
) {
    if !source.contains("skin_config.option") {
        return;
    }
    let mut matched_literal = false;
    for quote in ['"', '\''] {
        let pattern = format!("skin_config.option[{quote}");
        let mut rest = source;
        while let Some(start) = rest.find(&pattern) {
            let value_start = start + pattern.len();
            let after_start = &rest[value_start..];
            let Some(end) = after_start.find(quote) else {
                break;
            };
            let name = &after_start[..end];
            if let Some(option_id) =
                skin_config_options.get(name).and_then(|value| i32::try_from(*value).ok())
            {
                record_load_dependency_option(Some(dependencies), option_id, true);
                matched_literal = true;
            }
            rest = &after_start[end + quote.len_utf8()..];
        }
    }
    if !matched_literal {
        for option_id in skin_config_options.values().filter_map(|value| i32::try_from(*value).ok())
        {
            record_load_dependency_option(Some(dependencies), option_id, true);
        }
    }
}

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

#[derive(Debug, Clone)]
pub(super) struct MainStateProbe {
    pub(super) mode: MainStateProbeMode,
    pub(super) number_calls: Vec<i32>,
    pub(super) number_values: BTreeMap<i32, i32>,
    pub(super) option_calls: Vec<i32>,
    pub(super) option_values: BTreeMap<i32, bool>,
    pub(super) timer_calls: Vec<i32>,
    pub(super) timer_values: BTreeMap<i32, i32>,
    pub(super) event_index_calls: Vec<i32>,
    pub(super) event_index_values: BTreeMap<i32, i32>,
    pub(super) offset_values: BTreeMap<i32, LuaSkinOffsetValue>,
    pub(super) gauge_type_calls: usize,
    pub(super) gauge_type_value: i32,
    pub(super) float_number_calls: Vec<i32>,
    pub(super) float_number_values: BTreeMap<i32, f64>,
    pub(super) text_calls: Vec<i32>,
    pub(super) text_values: BTreeMap<i32, String>,
    pub(super) os_clock_calls: usize,
    pub(super) os_clock_value: Option<f64>,
    pub(super) time_value_us: i32,
    pub(super) next_dynamic_timer_id: i32,
    pub(super) dynamic_timers: Vec<(i32, String)>,
    pub(super) dynamic_timer_ids_by_observe: BTreeMap<String, i32>,
    pub(super) fixed_delay_timers: Vec<(i32, i32, i32)>,
    pub(super) unsupported_dynamic_timers: Vec<i32>,
    pub(super) load_time_constant_dynamic_timers: Vec<i32>,
    pub(super) next_runtime_flag_id: i32,
    pub(super) runtime_flags: Vec<LuaRuntimeFlagProbe>,
    pub(super) next_runtime_event_id: i32,
    pub(super) runtime_events: Vec<(i32, Vec<i32>)>,
    pub(super) runtime_event_ids_by_flags: BTreeMap<Vec<i32>, i32>,
    pub(super) capture_audio_actions: bool,
    pub(super) audio_actions: Vec<LuaAudioActionProbe>,
    pub(super) keylogger_destination_occurrences: BTreeMap<String, usize>,
    pub(super) gauge_lead_glow_occurrences: BTreeMap<String, usize>,
    pub(super) gauge_value_destination_occurrences: BTreeMap<String, usize>,
    pub(super) gauge_value_overlay_mode: Option<&'static str>,
    pub(super) result_panel_default: Option<i32>,
    pub(super) runtime_draw_paths: Vec<String>,
    pub(super) load_dependencies: Option<Arc<Mutex<SkinLoadDependencies>>>,
}

impl Default for MainStateProbe {
    fn default() -> Self {
        Self {
            mode: MainStateProbeMode::default(),
            number_calls: Vec::new(),
            number_values: BTreeMap::new(),
            option_calls: Vec::new(),
            option_values: BTreeMap::new(),
            timer_calls: Vec::new(),
            timer_values: BTreeMap::new(),
            event_index_calls: Vec::new(),
            event_index_values: BTreeMap::new(),
            offset_values: BTreeMap::new(),
            gauge_type_calls: 0,
            gauge_type_value: 0,
            float_number_calls: Vec::new(),
            float_number_values: BTreeMap::new(),
            text_calls: Vec::new(),
            text_values: BTreeMap::new(),
            os_clock_calls: 0,
            os_clock_value: None,
            time_value_us: 1_000_000,
            next_dynamic_timer_id: SKIN_DYNAMIC_TIMER_BASE,
            dynamic_timers: Vec::new(),
            dynamic_timer_ids_by_observe: BTreeMap::new(),
            fixed_delay_timers: Vec::new(),
            unsupported_dynamic_timers: Vec::new(),
            load_time_constant_dynamic_timers: Vec::new(),
            next_runtime_flag_id: 0,
            runtime_flags: Vec::new(),
            next_runtime_event_id: SKIN_EVENT_RUNTIME_BASE,
            runtime_events: Vec::new(),
            runtime_event_ids_by_flags: BTreeMap::new(),
            capture_audio_actions: true,
            audio_actions: Vec::new(),
            keylogger_destination_occurrences: BTreeMap::new(),
            gauge_lead_glow_occurrences: BTreeMap::new(),
            gauge_value_destination_occurrences: BTreeMap::new(),
            gauge_value_overlay_mode: None,
            result_panel_default: None,
            runtime_draw_paths: Vec::new(),
            load_dependencies: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) enum MainStateProbeMode {
    #[default]
    RuntimeStub,
    SymbolicNumbers {
        base_value: i32,
    },
    RecordNumbers {
        default_value: i32,
    },
}

impl MainStateProbe {
    pub(super) fn record_audio_action(
        &mut self,
        action: LuaAudioActionKindProbe,
        path: String,
        volume: f64,
    ) {
        if self.capture_audio_actions && !path.is_empty() && volume.is_finite() {
            self.audio_actions.push(LuaAudioActionProbe { action, path, volume });
        }
    }

    pub(super) fn take_audio_actions(&mut self) -> Vec<LuaAudioActionProbe> {
        std::mem::take(&mut self.audio_actions)
    }

    pub(super) fn clear_aux_calls(&mut self) {
        self.float_number_calls.clear();
        self.float_number_values.clear();
        self.text_calls.clear();
        self.text_values.clear();
        self.os_clock_calls = 0;
        self.os_clock_value = None;
        self.event_index_calls.clear();
        self.event_index_values.clear();
    }

    pub(super) fn begin_number_recording(&mut self, default_value: i32) {
        self.mode = MainStateProbeMode::SymbolicNumbers { base_value: default_value };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.clear_aux_calls();
    }

    pub(super) fn begin_number_call_recording(&mut self, default_value: i32) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.clear_aux_calls();
    }

    pub(super) fn begin_number_call_recording_with_option_value(
        &mut self,
        default_value: i32,
        option_id: i32,
        option_value: bool,
    ) {
        self.begin_number_call_recording(default_value);
        self.option_values.insert(option_id, option_value);
    }

    pub(super) fn begin_number_recording_with_value(&mut self, ref_id: i32, value: i32) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.number_values.insert(ref_id, value);
    }

    pub(super) fn begin_number_recording_with_values(&mut self, values: BTreeMap<i32, i32>) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values = values;
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
    }

    pub(super) fn begin_number_recording_with_values_and_options(
        &mut self,
        values: BTreeMap<i32, i32>,
        options: BTreeMap<i32, bool>,
    ) {
        self.begin_number_recording_with_values(values);
        self.option_values = options;
    }

    pub(super) fn begin_text_recording_with_values(&mut self, values: BTreeMap<i32, String>) {
        self.begin_number_recording_with_values(BTreeMap::new());
        self.text_calls.clear();
        self.text_values = values;
    }

    pub(super) fn begin_number_timer_recording_with_values(
        &mut self,
        values: BTreeMap<i32, i32>,
        mut timer_values: BTreeMap<i32, i32>,
    ) {
        self.begin_number_recording_with_values(values);
        timer_values.entry(i32::MIN).or_insert(i32::MIN);
        self.timer_values = timer_values;
    }

    pub(super) fn begin_option_call_recording(&mut self, default_value: bool) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.option_values.insert(i32::MIN, default_value);
    }

    pub(super) fn begin_option_recording_with_value(&mut self, option_id: i32, value: bool) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.option_values.insert(option_id, value);
    }

    pub(super) fn begin_timer_option_call_recording(&mut self) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.option_values.insert(i32::MIN, true);
        self.timer_values.insert(i32::MIN, i32::MIN);
    }

    pub(super) fn begin_timer_call_recording(&mut self, default_value: i32) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.timer_values.insert(i32::MIN, default_value);
    }

    pub(super) fn begin_timer_recording_with_values(
        &mut self,
        mut timer_values: BTreeMap<i32, i32>,
    ) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        timer_values.entry(i32::MIN).or_insert(i32::MIN);
        self.timer_values = timer_values;
    }

    pub(super) fn begin_timer_event_recording_with_values(
        &mut self,
        timer_values: BTreeMap<i32, i32>,
        event_id: i32,
        event_value: i32,
    ) {
        self.begin_timer_recording_with_values(timer_values);
        self.event_index_values.insert(event_id, event_value);
    }

    pub(super) fn begin_timer_option_recording_with_values(
        &mut self,
        timer_id: i32,
        timer_value: i32,
        option_id: i32,
        option_value: bool,
    ) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.timer_values.insert(timer_id, timer_value);
        self.option_values.insert(option_id, option_value);
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
    }

    pub(super) fn begin_timer_options_recording_with_values(
        &mut self,
        timer_values: BTreeMap<i32, i32>,
        option_values: BTreeMap<i32, bool>,
    ) {
        self.begin_timer_recording_with_values(timer_values);
        self.option_values = option_values;
    }

    pub(super) fn begin_gauge_type_call_recording(&mut self, value: i32) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = value;
    }

    pub(super) fn begin_gauge_type_recording_with_value(&mut self, value: i32) {
        self.begin_gauge_type_call_recording(value);
    }

    pub(super) fn begin_event_index_call_recording(&mut self, default_value: i32) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.clear_aux_calls();
    }

    pub(super) fn begin_event_index_recording_with_value(&mut self, event_id: i32, value: i32) {
        self.begin_event_index_call_recording(0);
        self.event_index_values.insert(event_id, value);
    }

    pub(super) fn begin_event_index_options_recording_with_values(
        &mut self,
        event_id: i32,
        event_value: i32,
        option_values: BTreeMap<i32, bool>,
        default_option_value: bool,
    ) {
        self.begin_event_index_recording_with_value(event_id, event_value);
        self.option_values.insert(i32::MIN, default_option_value);
        self.option_values.extend(option_values);
    }

    pub(super) fn begin_os_clock_recording(&mut self, value: f64) {
        self.mode = MainStateProbeMode::RecordNumbers { default_value: 0 };
        self.number_calls.clear();
        self.number_values.clear();
        self.option_calls.clear();
        self.option_values.clear();
        self.timer_calls.clear();
        self.timer_values.clear();
        self.event_index_calls.clear();
        self.event_index_values.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.float_number_calls.clear();
        self.float_number_values.clear();
        self.text_calls.clear();
        self.os_clock_calls = 0;
        self.os_clock_value = Some(value);
    }

    pub(super) fn begin_os_clock_options_recording(
        &mut self,
        value: f64,
        option_values: &[(i32, bool)],
        default_option_value: bool,
    ) {
        self.begin_os_clock_recording(value);
        self.option_values.insert(i32::MIN, default_option_value);
        for &(option_id, option_value) in option_values {
            self.option_values.insert(option_id, option_value);
        }
    }

    pub(super) fn end_recording(&mut self) {
        self.mode = MainStateProbeMode::RuntimeStub;
        self.number_values.clear();
        self.option_values.clear();
        self.timer_values.clear();
        self.event_index_values.clear();
        self.event_index_calls.clear();
        self.gauge_type_calls = 0;
        self.gauge_type_value = 0;
        self.os_clock_value = None;
        self.text_values.clear();
    }

    pub(super) fn number(&mut self, ref_id: i32) -> i32 {
        match self.mode {
            MainStateProbeMode::RuntimeStub => {
                let value = self
                    .number_values
                    .get(&ref_id)
                    .copied()
                    .unwrap_or_else(|| lua_runtime_stub_number(ref_id));
                self.record_load_time_number_dependency(ref_id, value);
                value
            }
            MainStateProbeMode::SymbolicNumbers { base_value } => {
                self.number_calls.push(ref_id);
                self.number_values.get(&ref_id).copied().unwrap_or(base_value + ref_id)
            }
            MainStateProbeMode::RecordNumbers { default_value } => {
                self.number_calls.push(ref_id);
                self.number_values.get(&ref_id).copied().unwrap_or(default_value)
            }
        }
    }

    pub(super) fn judge(&mut self, index: i32) -> i32 {
        main_state_judge_ref(index).map(|ref_id| self.number(ref_id)).unwrap_or(0)
    }

    pub(super) fn option(&mut self, option_id: i32) -> bool {
        if matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            let value = self
                .option_values
                .get(&option_id)
                .copied()
                .unwrap_or_else(|| lua_runtime_stub_option(option_id));
            self.record_load_time_option_dependency(option_id, value);
            return value;
        }
        self.option_calls.push(option_id);
        self.option_values
            .get(&option_id)
            .copied()
            .or_else(|| self.option_values.get(&i32::MIN).copied())
            .unwrap_or(false)
    }

    pub(super) fn record_load_time_number_dependency(&self, ref_id: i32, value: i32) {
        if let Some(dependencies) = &self.load_dependencies
            && let Ok(mut dependencies) = dependencies.lock()
        {
            dependencies.number_values.insert(ref_id, value);
        }
    }

    pub(super) fn record_load_time_option_dependency(&self, option_id: i32, value: bool) {
        if let Some(dependencies) = &self.load_dependencies
            && let Ok(mut dependencies) = dependencies.lock()
        {
            dependencies.option_values.insert(option_id, value);
        }
    }

    pub(super) fn record_load_time_event_index_dependency(&self, event_id: i32, value: i32) {
        if let Some(dependencies) = &self.load_dependencies
            && let Ok(mut dependencies) = dependencies.lock()
        {
            dependencies.event_index_values.insert(event_id, value);
        }
    }

    pub(super) fn record_load_time_text_dependency(&self, ref_id: i32, value: &str) {
        if let Some(dependencies) = &self.load_dependencies
            && let Ok(mut dependencies) = dependencies.lock()
        {
            dependencies.text_values.insert(ref_id, value.to_string());
        }
    }

    pub(super) fn offset(&self, offset_id: i32) -> LuaSkinOffsetValue {
        let value = self.offset_values.get(&offset_id).copied().unwrap_or_default();
        if matches!(self.mode, MainStateProbeMode::RuntimeStub)
            && let Some(dependencies) = &self.load_dependencies
            && let Ok(mut dependencies) = dependencies.lock()
        {
            dependencies.offset_id_values.insert(offset_id, value);
        }
        value
    }

    pub(super) fn timer(&mut self, timer_id: i32) -> i32 {
        if matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            return i32::MIN;
        }
        self.timer_calls.push(timer_id);
        self.timer_values
            .get(&timer_id)
            .copied()
            .or_else(|| self.timer_values.get(&i32::MIN).copied())
            .unwrap_or(i32::MIN)
    }

    pub(super) fn gauge_type(&mut self) -> i32 {
        if matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            return 0;
        }
        self.gauge_type_calls += 1;
        self.gauge_type_value
    }

    pub(super) fn float_number(&mut self, ref_id: i32) -> f64 {
        if matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            return 0.0;
        }
        self.float_number_calls.push(ref_id);
        self.float_number_values.get(&ref_id).copied().unwrap_or(0.0)
    }

    pub(super) fn volume_number(&mut self, ref_id: i32) -> f64 {
        if matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            return 1.0;
        }
        f64::from(self.number(ref_id)) / 100.0
    }

    pub(super) fn text(&mut self, ref_id: i32) -> String {
        if ref_id == 1010 {
            return format!("bmz-player {}", env!("CARGO_PKG_VERSION"));
        }
        if matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            if (1001..=1003).contains(&ref_id) {
                if let Some(value) = self.text_values.get(&ref_id).cloned() {
                    self.record_load_time_text_dependency(ref_id, &value);
                    return value;
                }
                return format!(
                    "{LUA_TEXT_REF_SENTINEL_PREFIX}{ref_id}{LUA_TEXT_REF_SENTINEL_SUFFIX}"
                );
            }
            let value = self.text_values.get(&ref_id).cloned().unwrap_or_default();
            self.record_load_time_text_dependency(ref_id, &value);
            return value;
        }
        self.text_calls.push(ref_id);
        self.text_values.get(&ref_id).cloned().unwrap_or_else(|| format!("Text{ref_id}"))
    }

    pub(super) fn event_index(&mut self, event_id: i32) -> i32 {
        match self.mode {
            MainStateProbeMode::RuntimeStub => {
                let value = self.event_index_values.get(&event_id).copied().unwrap_or_default();
                self.record_load_time_event_index_dependency(event_id, value);
                value
            }
            MainStateProbeMode::SymbolicNumbers { base_value } => {
                self.event_index_calls.push(event_id);
                self.event_index_values.get(&event_id).copied().unwrap_or(base_value + event_id)
            }
            MainStateProbeMode::RecordNumbers { default_value } => {
                self.event_index_calls.push(event_id);
                self.event_index_values.get(&event_id).copied().unwrap_or(default_value)
            }
        }
    }

    pub(super) fn time(&mut self) -> i32 {
        if matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            return lua_load_now_micros();
        }
        let value = self.time_value_us;
        self.time_value_us = self.time_value_us.saturating_add(1_000);
        value
    }

    pub(super) fn begin_draw_probe(
        &mut self,
        numbers: BTreeMap<i32, i32>,
        floats: BTreeMap<i32, f64>,
    ) {
        self.begin_number_recording_with_values(numbers);
        self.float_number_values = floats;
    }

    pub(super) fn os_clock(&mut self) -> f64 {
        if let Some(value) = self.os_clock_value {
            self.os_clock_calls += 1;
            return value;
        }
        if !matches!(self.mode, MainStateProbeMode::RuntimeStub) {
            self.os_clock_calls += 1;
            return 0.0;
        }
        lua_os_clock_seconds()
    }
}

pub(super) const LUA_TEXT_REF_SENTINEL_PREFIX: &str = "__BMZ_TEXT_REF_";
pub(super) const LUA_TEXT_REF_SENTINEL_SUFFIX: &str = "__";

pub(super) fn lua_runtime_stub_number(ref_id: i32) -> i32 {
    let now = unix_seconds_to_utc_datetime(lua_os_now_seconds());
    match ref_id {
        // beatoraja IntegerProperty: currenttime_year/month/day
        21 => now.year,
        22 => now.month as i32,
        23 => now.day as i32,
        _ => 0,
    }
}

pub(super) fn lua_runtime_stub_option(option_id: i32) -> bool {
    match option_id {
        // OPTION_AUTOPLAYOFF. Some Lua play skins build their score graph only for normal play.
        32 => true,
        _ => false,
    }
}

pub(super) fn create_main_state_stub(
    lua: &Lua,
    probe: Arc<Mutex<MainStateProbe>>,
) -> mlua::Result<Value> {
    let table = lua.create_table()?;
    table.set("timer_off_value", i32::MIN)?;
    let probe_for_number = probe.clone();
    table.set(
        "number",
        lua.create_function(move |_, ref_id: i32| {
            Ok(probe_for_number
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .number(ref_id))
        })?,
    )?;
    let probe_for_judge = probe.clone();
    table.set(
        "judge",
        lua.create_function(move |_, index: i32| {
            Ok(probe_for_judge
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .judge(index))
        })?,
    )?;
    let probe_for_option = probe.clone();
    let probe_for_timer = probe.clone();
    table.set(
        "option",
        lua.create_function(move |_, option_id: i32| {
            Ok(probe_for_option
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .option(option_id))
        })?,
    )?;
    let probe_for_text = probe.clone();
    table.set(
        "text",
        lua.create_function(move |_, ref_id: i32| {
            Ok(probe_for_text
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .text(ref_id))
        })?,
    )?;
    let probe_for_offset = probe.clone();
    table.set(
        "offset",
        lua.create_function(move |lua, offset_id: i32| {
            let value = probe_for_offset
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .offset(offset_id);
            create_main_state_offset_table(lua, value)
        })?,
    )?;
    let probe_for_float_number = probe.clone();
    table.set(
        "float_number",
        lua.create_function(move |_, ref_id: i32| {
            Ok(probe_for_float_number
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .float_number(ref_id))
        })?,
    )?;
    let probe_for_event_index = probe.clone();
    table.set(
        "event_index",
        lua.create_function(move |_, event_id: i32| {
            Ok(probe_for_event_index
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .event_index(event_id))
        })?,
    )?;
    table.set(
        "timer",
        lua.create_function(move |_, timer_id: i32| {
            Ok(probe_for_timer
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .timer(timer_id))
        })?,
    )?;
    let probe_for_time = probe.clone();
    table.set(
        "time",
        lua.create_function(move |_, ()| {
            Ok(probe_for_time
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .time())
        })?,
    )?;
    let probe_for_gauge_type = probe.clone();
    table.set(
        "gauge_type",
        lua.create_function(move |_, ()| {
            Ok(probe_for_gauge_type
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .gauge_type())
        })?,
    )?;
    let probe_for_volume_sys = probe.clone();
    table.set(
        "volume_sys",
        lua.create_function(move |_, ()| {
            Ok(probe_for_volume_sys
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .volume_number(57))
        })?,
    )?;
    let probe_for_volume_key = probe.clone();
    table.set(
        "volume_key",
        lua.create_function(move |_, ()| {
            Ok(probe_for_volume_key
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .volume_number(58))
        })?,
    )?;
    let probe_for_volume_bg = probe.clone();
    table.set(
        "volume_bg",
        lua.create_function(move |_, ()| {
            Ok(probe_for_volume_bg
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .volume_number(59))
        })?,
    )?;
    table.set("set_volume_sys", lua.create_function(|_, _: Value| Ok(true))?)?;
    table.set("set_volume_key", lua.create_function(|_, _: Value| Ok(true))?)?;
    table.set("set_volume_bg", lua.create_function(|_, _: Value| Ok(true))?)?;
    let probe_for_audio_play = probe.clone();
    table.set(
        "audio_play",
        lua.create_function(move |_, (path, volume): (Value, Value)| {
            if let Some((path, volume)) = lua_audio_path_and_volume(path, volume) {
                probe_for_audio_play
                    .lock()
                    .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                    .record_audio_action(LuaAudioActionKindProbe::Play, path, volume);
            }
            Ok(true)
        })?,
    )?;
    let probe_for_audio_loop = probe.clone();
    table.set(
        "audio_loop",
        lua.create_function(move |_, (path, volume): (Value, Value)| {
            if let Some((path, volume)) = lua_audio_path_and_volume(path, volume) {
                probe_for_audio_loop
                    .lock()
                    .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                    .record_audio_action(LuaAudioActionKindProbe::Loop, path, volume);
            }
            Ok(true)
        })?,
    )?;
    table.set(
        "audio_stop",
        lua.create_function(move |_, path: Value| {
            if let Value::String(path) = path
                && let Ok(path) = path.to_str()
            {
                probe
                    .lock()
                    .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                    .record_audio_action(LuaAudioActionKindProbe::Stop, path.to_string(), 1.0);
            }
            Ok(true)
        })?,
    )?;
    Ok(Value::Table(table))
}

pub(super) fn lua_audio_path_and_volume(path: Value, volume: Value) -> Option<(String, f64)> {
    let Value::String(path) = path else { return None };
    let volume = match volume {
        Value::Integer(volume) => volume as f64,
        Value::Number(volume) if volume.is_finite() => volume,
        _ => return None,
    };
    Some((path.to_str().ok()?.to_string(), volume))
}

pub(super) fn create_main_state_offset_table(
    lua: &Lua,
    offset: LuaSkinOffsetValue,
) -> mlua::Result<Value> {
    let table = lua.create_table()?;
    table.set("x", offset.x)?;
    table.set("y", offset.y)?;
    table.set("w", offset.w)?;
    table.set("h", offset.h)?;
    table.set("r", offset.r)?;
    table.set("a", offset.a)?;
    Ok(Value::Table(table))
}

#[derive(Debug)]
pub(super) struct TimerObserveState {
    timer_value: i32,
}

pub(super) fn lua_load_now_micros() -> i32 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = ORIGIN.get_or_init(Instant::now);
    origin.elapsed().as_micros().min(i32::MAX as u128) as i32
}

pub(super) fn lua_load_now_ms() -> i32 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = ORIGIN.get_or_init(Instant::now);
    origin.elapsed().as_millis().min(i32::MAX as u128) as i32
}

pub(super) fn create_os_stub(lua: &Lua, probe: Arc<Mutex<MainStateProbe>>) -> mlua::Result<Value> {
    let table = lua.create_table()?;
    let probe_for_clock = probe.clone();
    table.set(
        "clock",
        lua.create_function(move |_, ()| {
            Ok(probe_for_clock
                .lock()
                .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                .os_clock())
        })?,
    )?;
    table.set(
        "date",
        lua.create_function(|lua, args: Variadic<Value>| {
            let format = args
                .first()
                .and_then(|value| match value {
                    Value::String(value) => Some(value.to_string_lossy()),
                    _ => None,
                })
                .unwrap_or_else(|| "%Y-%m-%d %H:%M:%S".to_string());
            let seconds = args
                .get(1)
                .and_then(|value| match value {
                    Value::Integer(value) => Some(*value),
                    Value::Number(value) => Some(*value as i64),
                    _ => None,
                })
                .unwrap_or_else(lua_os_now_seconds);
            let date = unix_seconds_to_utc_datetime(seconds);
            if format == "*t" || format == "!*t" {
                let result = lua.create_table()?;
                result.set("year", date.year)?;
                result.set("month", date.month)?;
                result.set("day", date.day)?;
                result.set("hour", date.hour)?;
                result.set("min", date.minute)?;
                result.set("sec", date.second)?;
                result.set("wday", date.weekday)?;
                result.set("yday", date.yearday)?;
                result.set("isdst", false)?;
                Ok(Value::Table(result))
            } else {
                Ok(Value::String(lua.create_string(format_lua_date(&format, date))?))
            }
        })?,
    )?;
    Ok(Value::Table(table))
}

pub(super) fn create_io_stub(
    lua: &Lua,
    root: &Path,
    virtual_io_files: &BTreeMap<String, String>,
    load_dependencies: Option<Arc<Mutex<SkinLoadDependencies>>>,
) -> mlua::Result<Value> {
    let virtual_io_files =
        normalize_virtual_io_files(virtual_io_files).map_err(mlua::Error::external)?;
    let table = lua.create_table()?;
    let root_for_open = root.to_path_buf();
    let virtual_files_for_open = virtual_io_files.clone();
    let dependencies_for_open = load_dependencies.clone();
    table.set(
        "open",
        lua.create_function(move |lua, (path, mode): (String, Option<String>)| {
            let mode = mode.unwrap_or_else(|| "r".to_string());
            if matches!(mode.as_str(), "r" | "rb") {
                let Ok(requested) = normalize_skin_io_relative_path(&path) else {
                    return Ok(Value::Nil);
                };
                let virtual_source = virtual_files_for_open.get(&requested);
                record_virtual_io_dependency(
                    &requested,
                    virtual_source.map(String::as_str),
                    dependencies_for_open.as_ref(),
                );
                if let Some(source) = virtual_source {
                    return create_read_file_stub(lua, source.clone());
                }
                let Ok(path) = resolve_skin_io_path(&root_for_open, &requested) else {
                    mark_io_dependency_opaque(dependencies_for_open.as_ref());
                    return Ok(Value::Nil);
                };
                let Ok(source) = read_skin_io_source(&path) else {
                    mark_io_dependency_opaque(dependencies_for_open.as_ref());
                    return Ok(Value::Nil);
                };
                record_lua_loaded_file_dependency(&path, dependencies_for_open.as_ref());
                return create_read_file_stub(lua, source);
            }
            if mode.starts_with('w') || mode.starts_with('a') {
                return create_write_file_stub(lua);
            }
            Ok(Value::Nil)
        })?,
    )?;
    let root_for_lines = root.to_path_buf();
    let virtual_files_for_lines = virtual_io_files;
    let dependencies_for_lines = load_dependencies;
    table.set(
        "lines",
        lua.create_function(move |lua, path: String| {
            let Ok(requested) = normalize_skin_io_relative_path(&path) else {
                return create_lines_iterator(lua, Arc::new(Mutex::new(ReadFileState::default())));
            };
            let virtual_source = virtual_files_for_lines.get(&requested);
            record_virtual_io_dependency(
                &requested,
                virtual_source.map(String::as_str),
                dependencies_for_lines.as_ref(),
            );
            let source = if let Some(source) = virtual_source {
                source.clone()
            } else {
                let Ok(path) = resolve_skin_io_path(&root_for_lines, &requested) else {
                    mark_io_dependency_opaque(dependencies_for_lines.as_ref());
                    return create_lines_iterator(
                        lua,
                        Arc::new(Mutex::new(ReadFileState::default())),
                    );
                };
                let Ok(source) = read_skin_io_source(&path) else {
                    mark_io_dependency_opaque(dependencies_for_lines.as_ref());
                    return create_lines_iterator(
                        lua,
                        Arc::new(Mutex::new(ReadFileState::default())),
                    );
                };
                record_lua_loaded_file_dependency(&path, dependencies_for_lines.as_ref());
                source
            };
            create_lines_iterator(lua, Arc::new(Mutex::new(ReadFileState::new(source))))
        })?,
    )?;
    table.set(
        "close",
        lua.create_function(|_, file: Value| {
            let Value::Table(file) = file else {
                return Ok(false);
            };
            let close = file.get::<Function>("close")?;
            close.call::<bool>(file)
        })?,
    )?;
    Ok(Value::Table(table))
}

pub(super) fn lua_os_clock_seconds() -> f64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    let origin = ORIGIN.get_or_init(Instant::now);
    origin.elapsed().as_secs_f64()
}

#[derive(Debug, Default)]
pub(super) struct ReadFileState {
    source: String,
    cursor: usize,
    closed: bool,
}

impl ReadFileState {
    pub(super) fn new(source: String) -> Self {
        Self { source, cursor: 0, closed: false }
    }
}

pub(super) fn create_read_file_stub(lua: &Lua, source: String) -> mlua::Result<Value> {
    let file = lua.create_table()?;
    let state = Arc::new(Mutex::new(ReadFileState::new(source)));
    let state_for_read = state.clone();
    file.set(
        "read",
        lua.create_function(move |lua, (_self, format): (Value, Option<String>)| {
            let format = format.as_deref().unwrap_or("*l");
            let mut state = state_for_read
                .lock()
                .map_err(|_| mlua::Error::external("io read lock poisoned"))?;
            if state.closed {
                return Err(mlua::Error::external("attempt to use a closed file"));
            }
            match format {
                "*a" | "*all" => {
                    let rest = state.source[state.cursor..].to_string();
                    state.cursor = state.source.len();
                    Ok(Value::String(lua.create_string(rest)?))
                }
                "*l" => match read_file_line(&mut state) {
                    Some(line) => Ok(Value::String(lua.create_string(line)?)),
                    None => Ok(Value::Nil),
                },
                _ => Err(mlua::Error::external(format!(
                    "unsupported read format in Lua skin sandbox: {format}"
                ))),
            }
        })?,
    )?;
    let state_for_lines = state.clone();
    file.set(
        "lines",
        lua.create_function(move |lua, _: Value| {
            create_lines_iterator(lua, state_for_lines.clone())
        })?,
    )?;
    let state_for_close = state;
    file.set(
        "close",
        lua.create_function(move |_, _: Value| {
            let mut state = state_for_close
                .lock()
                .map_err(|_| mlua::Error::external("io close lock poisoned"))?;
            state.closed = true;
            Ok(true)
        })?,
    )?;
    Ok(Value::Table(file))
}

pub(super) fn create_lines_iterator(
    lua: &Lua,
    state: Arc<Mutex<ReadFileState>>,
) -> mlua::Result<Function> {
    lua.create_function(move |lua, ()| {
        let mut state =
            state.lock().map_err(|_| mlua::Error::external("io lines lock poisoned"))?;
        if state.closed {
            return Err(mlua::Error::external("attempt to use a closed file"));
        }
        let Some(line) = read_file_line(&mut state) else {
            return Ok(Value::Nil);
        };
        Ok(Value::String(lua.create_string(line)?))
    })
}

pub(super) fn read_file_line(state: &mut ReadFileState) -> Option<String> {
    if state.cursor >= state.source.len() {
        return None;
    }
    let rest = &state.source[state.cursor..];
    let end = rest.find('\n').unwrap_or(rest.len());
    let line = rest[..end].strip_suffix('\r').unwrap_or(&rest[..end]).to_string();
    state.cursor = state.cursor.saturating_add(end);
    if state.cursor < state.source.len() && state.source.as_bytes()[state.cursor] == b'\n' {
        state.cursor += 1;
    }
    Some(line)
}

pub(super) fn create_write_file_stub(lua: &Lua) -> mlua::Result<Value> {
    let file = lua.create_table()?;
    file.set(
        "write",
        lua.create_function(|_, (_self, _args): (Value, Variadic<Value>)| Ok(true))?,
    )?;
    file.set("close", lua.create_function(|_, _: Value| Ok(true))?)?;
    Ok(Value::Table(file))
}

pub(super) fn lua_os_now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LuaDateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    weekday: u32,
    yearday: u32,
}

pub(super) fn unix_seconds_to_utc_datetime(seconds: i64) -> LuaDateTime {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400) as u32;
    let (year, month, day) = civil_from_days(days);
    LuaDateTime {
        year,
        month,
        day,
        hour: seconds_of_day / 3_600,
        minute: (seconds_of_day % 3_600) / 60,
        second: seconds_of_day % 60,
        // Lua's wday is 1-based with Sunday == 1. 1970-01-01 was Thursday.
        weekday: ((days + 4).rem_euclid(7) + 1) as u32,
        yearday: yearday(year, month, day),
    }
}

pub(super) fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

pub(super) fn yearday(year: i32, month: u32, day: u32) -> u32 {
    const COMMON_MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut result = day;
    for m in 1..month {
        result += COMMON_MONTH_DAYS[(m - 1) as usize];
        if m == 2 && is_leap_year(year) {
            result += 1;
        }
    }
    result
}

pub(super) fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(super) fn format_lua_date(format: &str, date: LuaDateTime) -> String {
    let mut output = String::new();
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('Y') => output.push_str(&format!("{:04}", date.year)),
            Some('m') => output.push_str(&format!("{:02}", date.month)),
            Some('d') => output.push_str(&format!("{:02}", date.day)),
            Some('H') => output.push_str(&format!("{:02}", date.hour)),
            Some('M') => output.push_str(&format!("{:02}", date.minute)),
            Some('S') => output.push_str(&format!("{:02}", date.second)),
            Some('%') => output.push('%'),
            Some(other) => {
                output.push('%');
                output.push(other);
            }
            None => output.push('%'),
        }
    }
    output
}

#[derive(Debug)]
pub(super) struct EventObserveBoolState {
    is_on: bool,
}

#[derive(Debug)]
pub(super) struct EventObserveTimerState {
    value: i32,
}

#[derive(Debug)]
pub(super) struct EventMinIntervalState {
    last_execution_ms: Option<i32>,
}

/// beatoraja の `EventUtility` 相当。CustomEvent 用 callback 生成器を提供する。
pub(super) fn create_event_util_module(lua: &Lua) -> mlua::Result<Value> {
    let table = lua.create_table()?;

    table.set(
        "event_observe_turn_true",
        lua.create_function(|lua, (observed, action): (Function, Function)| {
            let state = Arc::new(Mutex::new(EventObserveBoolState { is_on: false }));
            lua.create_function(move |_, ()| {
                let on = observed.call::<bool>(())?;
                let mut state = state
                    .lock()
                    .map_err(|_| mlua::Error::external("event observe lock poisoned"))?;
                if state.is_on != on {
                    state.is_on = on;
                    if state.is_on {
                        action.call::<()>(())?;
                    }
                }
                Ok(true)
            })
        })?,
    )?;

    table.set(
        "event_observe_timer",
        lua.create_function(|lua, (timer, action): (Function, Function)| {
            let state = Arc::new(Mutex::new(EventObserveTimerState { value: TIMER_OFF_VALUE }));
            lua.create_function(move |_, ()| {
                let value = timer.call::<i32>(())?;
                let mut state =
                    state.lock().map_err(|_| mlua::Error::external("event timer lock poisoned"))?;
                if value != state.value && value != TIMER_OFF_VALUE {
                    state.value = value;
                    action.call::<()>(())?;
                }
                Ok(true)
            })
        })?,
    )?;

    table.set(
        "event_observe_timer_on",
        lua.create_function(|lua, (timer, action): (Function, Function)| {
            let state = Arc::new(Mutex::new(EventObserveBoolState { is_on: false }));
            lua.create_function(move |_, ()| {
                let on = timer.call::<i32>(())? != TIMER_OFF_VALUE;
                let mut state = state
                    .lock()
                    .map_err(|_| mlua::Error::external("event timer-on lock poisoned"))?;
                if state.is_on != on {
                    state.is_on = on;
                    if state.is_on {
                        action.call::<()>(())?;
                    }
                }
                Ok(true)
            })
        })?,
    )?;

    table.set(
        "event_observe_timer_off",
        lua.create_function(|lua, (timer, action): (Function, Function)| {
            let state = Arc::new(Mutex::new(EventObserveBoolState { is_on: true }));
            lua.create_function(move |_, ()| {
                let off = timer.call::<i32>(())? == TIMER_OFF_VALUE;
                let mut state = state
                    .lock()
                    .map_err(|_| mlua::Error::external("event timer-off lock poisoned"))?;
                if state.is_on != off {
                    state.is_on = off;
                    if state.is_on {
                        action.call::<()>(())?;
                    }
                }
                Ok(true)
            })
        })?,
    )?;

    table.set(
        "event_min_interval",
        lua.create_function(|lua, (min_interval_ms, action): (i32, Function)| {
            let state = Arc::new(Mutex::new(EventMinIntervalState { last_execution_ms: None }));
            lua.create_function(move |_, ()| {
                let now = lua_load_now_ms();
                let mut state = state
                    .lock()
                    .map_err(|_| mlua::Error::external("event interval lock poisoned"))?;
                let should_run = state
                    .last_execution_ms
                    .is_none_or(|last| now.saturating_sub(last) >= min_interval_ms);
                if should_run {
                    state.last_execution_ms = Some(now);
                    action.call::<()>(())?;
                }
                Ok(true)
            })
        })?,
    )?;

    Ok(Value::Table(table))
}

pub(super) fn create_luajava_stub(lua: &Lua) -> mlua::Result<Value> {
    let table = lua.create_table()?;
    table.set(
        "bindClass",
        lua.create_function(|lua, class_name: String| match class_name.as_str() {
            "com.badlogic.gdx.Gdx" => create_luajava_gdx_stub(lua),
            "com.badlogic.gdx.controllers.Controllers" => create_luajava_controllers_stub(lua),
            _ => create_luajava_object_stub(lua),
        })?,
    )?;
    table.set(
        "newInstance",
        lua.create_function(|lua, (_class_name, _args): (String, Variadic<Value>)| {
            create_luajava_object_stub(lua)
        })?,
    )?;
    table.set(
        "createProxy",
        lua.create_function(|lua, _: Variadic<Value>| create_luajava_object_stub(lua))?,
    )?;
    Ok(Value::Table(table))
}

pub(super) fn create_luajava_gdx_stub(lua: &Lua) -> mlua::Result<Value> {
    let gdx = lua.create_table()?;
    let input = lua.create_table()?;
    input
        .set("isKeyPressed", lua.create_function(|_, (_self, _key): (Value, Value)| Ok(false))?)?;
    gdx.set("input", input)?;
    Ok(Value::Table(gdx))
}

pub(super) fn create_luajava_controllers_stub(lua: &Lua) -> mlua::Result<Value> {
    let controllers = lua.create_table()?;
    controllers.set(
        "getControllers",
        lua.create_function(|lua, _self: Value| {
            let list = lua.create_table()?;
            // libGDX Array exposes `size` as a numeric field. Returning an
            // empty list is the neutral load-time value and prevents skins
            // from treating the generic truthy object stub as live input.
            list.set("size", 0)?;
            list.set(
                "get",
                lua.create_function(|_, (_self, _index): (Value, Value)| Ok(Value::Nil))?,
            )?;
            Ok(Value::Table(list))
        })?,
    )?;
    Ok(Value::Table(controllers))
}

pub(super) fn create_luajava_object_stub(lua: &Lua) -> mlua::Result<Value> {
    let object = lua.create_table()?;
    let metatable = lua.create_table()?;
    metatable.set(
        "__index",
        lua.create_function(|lua, (_table, _key): (Value, Value)| create_luajava_object_stub(lua))?,
    )?;
    metatable.set(
        "__call",
        lua.create_function(|lua, (_self, _args): (Value, Variadic<Value>)| {
            create_luajava_object_stub(lua)
        })?,
    )?;
    object.set_metatable(Some(metatable));
    Ok(Value::Table(object))
}

/// beatoraja の `TimerUtility` 相当。Lua スキンが `require("timer_util")` できるようにする。
pub(super) fn create_timer_util_module(
    lua: &Lua,
    probe: Arc<Mutex<MainStateProbe>>,
) -> mlua::Result<Value> {
    let table = lua.create_table()?;

    table.set(
        "now_timer",
        lua.create_function(|_, timer_value: i32| {
            Ok(if timer_value != TIMER_OFF_VALUE {
                lua_load_now_micros().saturating_sub(timer_value.max(0))
            } else {
                0
            })
        })?,
    )?;
    table.set(
        "is_timer_on",
        lua.create_function(|_, timer_value: i32| Ok(timer_value != TIMER_OFF_VALUE))?,
    )?;
    table.set(
        "is_timer_off",
        lua.create_function(|_, timer_value: i32| Ok(timer_value == TIMER_OFF_VALUE))?,
    )?;

    let probe_for_timer_function = probe.clone();
    table.set(
        "timer_function",
        lua.create_function(move |lua, timer_id: i32| {
            let probe = probe_for_timer_function.clone();
            lua.create_function(move |_, _: Value| {
                Ok(probe
                    .lock()
                    .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?
                    .timer(timer_id))
            })
        })?,
    )?;

    let probe_for_observe = probe.clone();
    table.set(
        "timer_observe_boolean",
        lua.create_function(move |lua, observed: Function| {
            let specialized = infer_is_gauge_iidx_global_observe(lua, &observed);
            let observe = specialized
                .clone()
                .or_else(|| infer_runtime_boolean_field_observe(lua, &observed, &probe_for_observe))
                .or_else(|| infer_boolean_predicate(&observed, &probe_for_observe, None));
            let unsupported = observe.is_none();
            let load_time_constant = specialized.is_none()
                && observe.as_deref().is_some_and(is_constant_boolean_condition);
            let observe = observe.unwrap_or_else(|| "number(0) < 0".to_string());
            let timer_id = {
                let mut probe = probe_for_observe
                    .lock()
                    .map_err(|_| mlua::Error::external("main_state probe lock poisoned"))?;
                if !unsupported
                    && let Some(timer_id) =
                        probe.dynamic_timer_ids_by_observe.get(&observe).copied()
                {
                    timer_id
                } else {
                    let timer_id = probe.next_dynamic_timer_id;
                    probe.next_dynamic_timer_id += 1;
                    probe.dynamic_timers.push((timer_id, observe.clone()));
                    if !unsupported {
                        probe.dynamic_timer_ids_by_observe.insert(observe, timer_id);
                    }
                    if unsupported {
                        probe.unsupported_dynamic_timers.push(timer_id);
                    }
                    if load_time_constant {
                        probe.load_time_constant_dynamic_timers.push(timer_id);
                    }
                    timer_id
                }
            };
            let state = Arc::new(Mutex::new(TimerObserveState { timer_value: TIMER_OFF_VALUE }));
            let observed_for_timer = observed.clone();
            let inner = lua.create_function(move |_, ()| {
                let on = observed_for_timer.call::<bool>(())?;
                let mut state = state
                    .lock()
                    .map_err(|_| mlua::Error::external("timer observe lock poisoned"))?;
                if on && state.timer_value == TIMER_OFF_VALUE {
                    state.timer_value = lua_load_now_ms();
                } else if !on && state.timer_value != TIMER_OFF_VALUE {
                    state.timer_value = TIMER_OFF_VALUE;
                }
                Ok(state.timer_value)
            })?;
            let map: Table = lua.globals().get("bmz_timer_fn_map")?;
            map.set(inner.clone(), timer_id)?;
            Ok(inner)
        })?,
    )?;

    table.set(
        "new_passive_timer",
        lua.create_function(|lua, ()| {
            let state = Arc::new(Mutex::new(TimerObserveState { timer_value: TIMER_OFF_VALUE }));
            let passive = lua.create_table()?;
            let state_for_timer = state.clone();
            passive.set(
                "timer",
                lua.create_function(move |_, ()| {
                    Ok(state_for_timer
                        .lock()
                        .map_err(|_| mlua::Error::external("passive timer lock poisoned"))?
                        .timer_value)
                })?,
            )?;
            let state_for_turn_on = state.clone();
            passive.set(
                "turn_on",
                lua.create_function(move |_, ()| {
                    let mut state = state_for_turn_on
                        .lock()
                        .map_err(|_| mlua::Error::external("passive timer lock poisoned"))?;
                    if state.timer_value == TIMER_OFF_VALUE {
                        state.timer_value = lua_load_now_micros();
                    }
                    Ok(())
                })?,
            )?;
            let state_for_turn_on_reset = state.clone();
            passive.set(
                "turn_on_reset",
                lua.create_function(move |_, ()| {
                    state_for_turn_on_reset
                        .lock()
                        .map_err(|_| mlua::Error::external("passive timer lock poisoned"))?
                        .timer_value = lua_load_now_micros();
                    Ok(())
                })?,
            )?;
            passive.set(
                "turn_off",
                lua.create_function(move |_, ()| {
                    state
                        .lock()
                        .map_err(|_| mlua::Error::external("passive timer lock poisoned"))?
                        .timer_value = TIMER_OFF_VALUE;
                    Ok(())
                })?,
            )?;
            Ok(passive)
        })?,
    )?;

    Ok(Value::Table(table))
}
