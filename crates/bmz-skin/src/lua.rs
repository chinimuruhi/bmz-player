use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CStr;
use std::fs;
use std::os::raw::c_int;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{fmt, panic};

use anyhow::{Context, Result, anyhow, bail};
use mlua::{Function, HookTriggers, Lua, RegistryKey, Table, Value, Variadic, VmState};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};

use bmz_skin_document::{
    SKIN_DYNAMIC_TIMER_BASE, SKIN_EVENT_RESULT_PANEL_GRAPH, SKIN_EVENT_RESULT_PANEL_IR,
    SKIN_EVENT_RUNTIME_BASE, SKIN_EXPR_ADJUSTED_COVER, SKIN_EXPR_ADJUSTED_RATE,
    SKIN_EXPR_ADJUSTED_RATE_ADOT, SKIN_EXPR_COURSE_CLEAR_RATE, SKIN_EXPR_COURSE_TABLE_TEXT,
    SKIN_EXPR_FAST_SLOW_BREAKDOWN_HEIGHT, SKIN_EXPR_FS_THRESHOLD, SKIN_EXPR_GAUGE_AMOUNT_FRACTION,
    SKIN_EXPR_GAUGE_AMOUNT_INTEGER, SKIN_EXPR_GAUGE_PERCENT_FRACTION,
    SKIN_EXPR_GAUGE_PERCENT_INTEGER, SKIN_EXPR_RESULT_TABLE_TITLE, SKIN_REF_PLAY_GAUGE_TYPE,
};

use crate::{
    LoadedLuaSkinValue, LuaLoadRuntimeState, LuaMainState, LuaSkinOffsetValue,
    SkinLoadDependencies, SkinLoadWarning, SkinLoadedFileDependency,
};

mod conversion;
mod function_inference;
mod sandbox;

use conversion::*;
use function_inference::*;
use sandbox::*;

const LUA_INSTRUCTION_LIMIT: i64 = 2_000_000;
const LUA_INFERENCE_INSTRUCTION_LIMIT: i64 = 16_000_000;
const LUA_HOOK_INTERVAL: u32 = 1_000;
const LUA_MAX_TABLE_DEPTH: usize = 64;
const LUA_MAX_TABLE_ENTRIES: usize = 200_000;
const LUA_IO_MAX_READ_BYTES: usize = 8 * 1024 * 1024;
const LUA_MEMORY_LIMIT_BYTES: usize = 256 * 1024 * 1024;
const TIMER_OFF_VALUE: i32 = i32::MIN;
pub const LUA_DRAW_CALLBACK_PREFIX: &str = "bmz:lua_draw_callback:";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LuaRuntimeFlagProbe {
    id: i32,
    table: String,
    field: String,
    initial: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum LuaRuntimeScalar {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LuaAudioActionKindProbe {
    Play,
    Loop,
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
struct LuaAudioActionProbe {
    action: LuaAudioActionKindProbe,
    path: String,
    volume: f64,
}

/// beatoraja fast/slow 判定カウント ref (graph 比率推論用)
const FAST_SLOW_FAST_REFS: [i32; 6] = [410, 412, 414, 416, 418, 421];
const FAST_SLOW_SLOW_REFS: [i32; 6] = [411, 413, 415, 417, 419, 422];

fn main_state_judge_ref(index: i32) -> Option<i32> {
    match index {
        0 => Some(110),
        1 => Some(111),
        2 => Some(112),
        3 => Some(113),
        4 => Some(114),
        5 => Some(420),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertReport {
    pub warnings: Vec<String>,
}

struct LuaRuntimeCallback {
    path: String,
    key: Option<RegistryKey>,
}

/// A Lua-only sidecar that owns the runtime VM and every callback registry key.
///
/// The VM is intentionally not cloneable. Its callbacks are obtained by a second
/// load after inference has completed, so inference can never mutate runtime
/// closure state, module state, or the Lua random-number generator.
pub struct LuaSkinRuntime {
    lua: Lua,
    callbacks: Vec<LuaRuntimeCallback>,
    main_state_key: RegistryKey,
    instruction_budget: LuaInstructionBudget,
    skin_path: PathBuf,
    failed_callbacks: BTreeSet<usize>,
    failure_log_count: usize,
}

impl fmt::Debug for LuaSkinRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LuaSkinRuntime")
            .field("skin_path", &self.skin_path)
            .field("callback_count", &self.callbacks.len())
            .field("failed_callbacks", &self.failed_callbacks)
            .finish_non_exhaustive()
    }
}

impl LuaSkinRuntime {
    pub fn callback_count(&self) -> usize {
        self.callbacks.len()
    }

    pub fn callback_path(&self, callback_id: usize) -> Option<&str> {
        self.callbacks.get(callback_id).map(|callback| callback.path.as_str())
    }

    /// Number of callback failures that produced a diagnostic. Repeated failures
    /// of the same callback remain log-once and do not increase this value.
    pub fn failure_log_count(&self) -> usize {
        self.failure_log_count
    }

    pub fn evaluate_draw(&mut self, callback_id: usize, state: &dyn LuaMainState) -> bool {
        self.instruction_budget.begin_runtime_callback();
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.evaluate_draw_inner(callback_id, state)
        }));
        match result {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                self.log_callback_failure_once(callback_id, &error.to_string());
                false
            }
            Err(_) => {
                self.log_callback_failure_once(callback_id, "panic while executing Lua callback");
                false
            }
        }
    }

    fn evaluate_draw_inner(
        &self,
        callback_id: usize,
        state: &dyn LuaMainState,
    ) -> mlua::Result<bool> {
        let callback = self.callbacks.get(callback_id).ok_or_else(|| {
            mlua::Error::runtime(format!("unknown Lua draw callback ID {callback_id}"))
        })?;
        let key = callback.key.as_ref().ok_or_else(|| {
            mlua::Error::runtime(format!(
                "Lua draw callback was not registered at {}",
                callback.path
            ))
        })?;
        let function: Function = self.lua.registry_value(key)?;
        let main_state: Table = self.lua.registry_value(&self.main_state_key)?;

        self.lua.scope(|scope| {
            const FIELDS: &[&str] = &[
                "option",
                "number",
                "float",
                "float_number",
                "text",
                "timer",
                "event_index",
                "gauge_type",
                "time",
                "judge",
                "offset",
            ];
            let originals = FIELDS
                .iter()
                .map(|field| Ok((*field, main_state.get::<Value>(*field)?)))
                .collect::<mlua::Result<Vec<_>>>()?;

            main_state.set("option", scope.create_function(|_, id: i32| Ok(state.option(id)))?)?;
            main_state.set("number", scope.create_function(|_, id: i32| Ok(state.number(id)))?)?;
            let float = scope.create_function(|_, id: i32| Ok(state.float(id)))?;
            main_state.set("float", float.clone())?;
            main_state.set("float_number", float)?;
            main_state.set("text", scope.create_function(|_, id: i32| Ok(state.text(id)))?)?;
            main_state.set(
                "timer",
                scope
                    .create_function(|_, id: i32| Ok(state.timer(id).unwrap_or(TIMER_OFF_VALUE)))?,
            )?;
            main_state.set(
                "event_index",
                scope.create_function(|_, id: i32| Ok(state.event_index(id)))?,
            )?;
            main_state.set("gauge_type", scope.create_function(|_, ()| Ok(state.gauge_type()))?)?;
            main_state.set("time", scope.create_function(|_, ()| Ok(state.time_us()))?)?;
            main_state
                .set("judge", scope.create_function(|_, index: i32| Ok(state.judge(index)))?)?;
            main_state.set(
                "offset",
                scope.create_function(|lua, id: i32| {
                    create_main_state_offset_table(lua, state.offset(id))
                })?,
            )?;

            let result = match function.call::<Value>(()) {
                Ok(Value::Boolean(value)) => Ok(value),
                Ok(Value::Nil) => Err(mlua::Error::runtime("Lua draw callback returned nil")),
                Ok(value) => Err(mlua::Error::runtime(format!(
                    "Lua draw callback returned {}, expected boolean",
                    value.type_name()
                ))),
                Err(error) => Err(error),
            };

            // Scoped functions borrow the current frame snapshot. Restore the
            // persistent load-time stubs before the scope invalidates them.
            for (field, value) in originals {
                main_state.set(field, value)?;
            }
            result
        })
    }

    fn log_callback_failure_once(&mut self, callback_id: usize, error: &str) {
        if !self.failed_callbacks.insert(callback_id) {
            return;
        }
        self.failure_log_count = self.failure_log_count.saturating_add(1);
        let path = self.callback_path(callback_id).unwrap_or("<unknown>");
        tracing::warn!(
            skin = %self.skin_path.display(),
            callback_id,
            field_path = path,
            classification = "ERROR",
            error,
            "Lua draw callback failed; falling back to false"
        );
    }
}

pub fn load_lua_skin_value(
    input: &Path,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
    virtual_io_files: &BTreeMap<String, String>,
) -> Result<LoadedLuaSkinValue> {
    let ExecutedLuaSkin { value, warnings, files, dependencies, lua_runtime, runtime_draw_paths } =
        execute_lua_skin(input, options, files, runtime_state, virtual_io_files)?;
    Ok(LoadedLuaSkinValue {
        value,
        lua_runtime,
        runtime_draw_paths,
        warnings: warnings.into_iter().map(|message| SkinLoadWarning { message }).collect(),
        files,
        dependencies,
        internal_enabled_options: Vec::new(),
    })
}

pub fn load_lua_skin_header_value(input: &Path) -> Result<LoadedLuaSkinValue> {
    let (value, warnings) = execute_lua_skin_header(input)?;
    Ok(LoadedLuaSkinValue {
        value,
        lua_runtime: None,
        runtime_draw_paths: Vec::new(),
        warnings: warnings.into_iter().map(|message| SkinLoadWarning { message }).collect(),
        files: BTreeMap::new(),
        dependencies: SkinLoadDependencies::default(),
        internal_enabled_options: Vec::new(),
    })
}

pub fn convert_lua_skin_to_json(
    input: &Path,
    output: &Path,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
) -> Result<ConvertReport> {
    let ExecutedLuaSkin { value: json, warnings, runtime_draw_paths, .. } =
        execute_lua_skin(input, options, files, &LuaLoadRuntimeState::default(), &BTreeMap::new())?;
    if !runtime_draw_paths.is_empty() {
        bail!(
            "lua-to-json cannot serialize runtime draw callbacks: {}",
            runtime_draw_paths.join(", ")
        );
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output dir: {}", parent.display()))?;
    }
    fs::write(output, serde_json::to_string_pretty(&json)? + "\n")
        .with_context(|| format!("failed to write json skin: {}", output.display()))?;

    Ok(ConvertReport { warnings })
}

fn execute_lua_skin_header(input: &Path) -> Result<(JsonValue, Vec<String>)> {
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

    let lua = Lua::new();
    let instruction_budget = install_instruction_limit(&lua);
    let probe = install_sandbox(
        &lua,
        &root,
        &BTreeMap::new(),
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        &BTreeMap::new(),
        None,
    )?;
    let header = lua
        .load(&source)
        .set_name(input.to_string_lossy().as_ref())
        .eval::<Value>()
        .with_context(|| format!("failed to execute lua skin header: {}", input.display()))?;
    instruction_budget.begin_inference();
    let header_json = lua_value_to_json(
        &lua,
        header,
        "$",
        0,
        &mut warnings,
        &probe,
        &instruction_budget,
        &mut table_budget,
    )?;

    Ok((header_json, warnings))
}

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

fn normalize_lua_skin_audio_paths(
    skin_root: &Path,
    root: &mut JsonMap<String, JsonValue>,
    warnings: &mut Vec<String>,
) {
    if let Some(JsonValue::Array(actions)) = root.get_mut("sceneAudio") {
        normalize_lua_skin_audio_action_array(skin_root, actions, warnings);
    }
    if let Some(JsonValue::Array(events)) = root.get_mut("customEvents") {
        for event in events {
            let JsonValue::Object(event) = event else { continue };
            if let Some(JsonValue::Array(actions)) = event.get_mut("audioActions") {
                normalize_lua_skin_audio_action_array(skin_root, actions, warnings);
            }
        }
    }
}

fn normalize_lua_skin_audio_action_array(
    skin_root: &Path,
    actions: &mut Vec<JsonValue>,
    warnings: &mut Vec<String>,
) {
    actions.retain_mut(|action| {
        let JsonValue::Object(action) = action else { return false };
        let Some(JsonValue::String(path)) = action.get_mut("path") else { return false };
        let requested = path.clone();
        let requested_path = Path::new(&requested);
        let candidate = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            skin_root.join(requested_path)
        };
        let Ok(candidate) = canonicalize_skin_path(&candidate) else {
            warnings.push(format!("skipping missing skin audio path: {requested}"));
            return false;
        };
        let Ok(relative) = candidate.strip_prefix(skin_root) else {
            warnings.push(format!("skipping skin audio path outside skin root: {requested}"));
            return false;
        };
        *path = relative.to_string_lossy().replace('\\', "/");
        true
    });
}

fn lua_result_panel_value(value: Value) -> Option<i32> {
    match value {
        Value::Integer(value) => i32::try_from(value).ok(),
        Value::Number(value) if value.is_finite() && value.fract() == 0.0 => Some(value as i32),
        _ => None,
    }
    .filter(|panel| (0..=2).contains(panel))
}

fn result_panel_from_local_mode(mode: i32) -> Option<i32> {
    match mode {
        0 => Some(2),
        1 => Some(1),
        _ => None,
    }
}

fn record_local_result_panel_default(
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    mode: i32,
) -> Option<()> {
    let panel = result_panel_from_local_mode(mode)?;
    let mut probe = main_state_probe.lock().ok()?;
    probe.result_panel_default.get_or_insert(panel);
    Some(())
}

/// Returns the index and integer value of a closure upvalue named `result_mode`.
///
/// Lua 5.4 does not expose arbitrary upvalues through mlua's safe API. This
/// private C callback only inspects the function passed as argument 1 and never
/// installs the debug library into the skin sandbox.
unsafe extern "C-unwind" fn find_result_mode_upvalue(state: *mut mlua::lua_State) -> c_int {
    // SAFETY: mlua invokes this callback with a live Lua state. Every inspected
    // stack slot belongs to this call, and `lua_getupvalue` pushes exactly one
    // value whenever it returns a non-null name.
    unsafe {
        if mlua::ffi::lua_type(state, 1) != mlua::ffi::LUA_TFUNCTION {
            return 0;
        }
        for index in 1..=255 {
            let name = mlua::ffi::lua_getupvalue(state, 1, index);
            if name.is_null() {
                break;
            }
            let matches = CStr::from_ptr(name).to_bytes() == b"result_mode";
            if matches && mlua::ffi::lua_isinteger(state, -1) != 0 {
                let value = mlua::ffi::lua_tointeger(state, -1);
                mlua::ffi::lua_pop(state, 1);
                mlua::ffi::lua_pushinteger(state, i64::from(index));
                mlua::ffi::lua_pushinteger(state, value);
                return 2;
            }
            mlua::ffi::lua_pop(state, 1);
        }
        0
    }
}

/// Returns the index and boolean value of a closure upvalue named `flag_score`.
///
/// mz-select keeps its score-availability guard local to the player-data
/// module, while Luxe Flat exposes the same guard as a global. Inspecting the
/// closure lets both original skins produce the same runtime draw predicate.
unsafe extern "C-unwind" fn find_flag_score_upvalue(state: *mut mlua::lua_State) -> c_int {
    // SAFETY: see `find_result_mode_upvalue`; this callback only inspects the
    // function passed in stack slot 1 and balances every pushed upvalue.
    unsafe {
        if mlua::ffi::lua_type(state, 1) != mlua::ffi::LUA_TFUNCTION {
            return 0;
        }
        for index in 1..=255 {
            let name = mlua::ffi::lua_getupvalue(state, 1, index);
            if name.is_null() {
                break;
            }
            let matches = CStr::from_ptr(name).to_bytes() == b"flag_score";
            if matches && mlua::ffi::lua_type(state, -1) == mlua::ffi::LUA_TBOOLEAN {
                let value = mlua::ffi::lua_toboolean(state, -1);
                mlua::ffi::lua_pop(state, 1);
                mlua::ffi::lua_pushinteger(state, i64::from(index));
                mlua::ffi::lua_pushboolean(state, value);
                return 2;
            }
            mlua::ffi::lua_pop(state, 1);
        }
        0
    }
}

/// Replaces one integer closure upvalue and reports whether the index existed.
unsafe extern "C-unwind" fn set_integer_upvalue(state: *mut mlua::lua_State) -> c_int {
    // SAFETY: arguments are validated before touching the stack. `lua_setupvalue`
    // consumes the pushed value and only mutates the function passed to this call.
    unsafe {
        if mlua::ffi::lua_type(state, 1) != mlua::ffi::LUA_TFUNCTION
            || mlua::ffi::lua_isinteger(state, 2) == 0
            || mlua::ffi::lua_isinteger(state, 3) == 0
        {
            mlua::ffi::lua_pushboolean(state, 0);
            return 1;
        }
        let index = mlua::ffi::lua_tointeger(state, 2);
        let value = mlua::ffi::lua_tointeger(state, 3);
        let Ok(index) = c_int::try_from(index) else {
            mlua::ffi::lua_pushboolean(state, 0);
            return 1;
        };
        mlua::ffi::lua_pushinteger(state, value);
        let name = mlua::ffi::lua_setupvalue(state, 1, index);
        mlua::ffi::lua_pushboolean(state, if name.is_null() { 0 } else { 1 });
        1
    }
}

/// Replaces one boolean closure upvalue and reports whether the index existed.
unsafe extern "C-unwind" fn set_boolean_upvalue(state: *mut mlua::lua_State) -> c_int {
    // SAFETY: arguments are validated before touching the stack. `lua_setupvalue`
    // consumes the pushed value and only mutates the supplied function.
    unsafe {
        if mlua::ffi::lua_type(state, 1) != mlua::ffi::LUA_TFUNCTION
            || mlua::ffi::lua_isinteger(state, 2) == 0
            || mlua::ffi::lua_type(state, 3) != mlua::ffi::LUA_TBOOLEAN
        {
            mlua::ffi::lua_pushboolean(state, 0);
            return 1;
        }
        let index = mlua::ffi::lua_tointeger(state, 2);
        let value = mlua::ffi::lua_toboolean(state, 3);
        let Ok(index) = c_int::try_from(index) else {
            mlua::ffi::lua_pushboolean(state, 0);
            return 1;
        };
        mlua::ffi::lua_pushboolean(state, value);
        let name = mlua::ffi::lua_setupvalue(state, 1, index);
        mlua::ffi::lua_pushboolean(state, if name.is_null() { 0 } else { 1 });
        1
    }
}

fn lua_result_mode_upvalue(lua: &Lua, function: &Function) -> Option<(i32, i32)> {
    // SAFETY: both callbacks obey Lua's C function ABI and access only their
    // call frame. They are retained by mlua for the duration of `call`.
    let helper = unsafe { lua.create_c_function(find_result_mode_upvalue).ok()? };
    let (index, value) = helper.call::<(i64, i64)>(function.clone()).ok()?;
    Some((i32::try_from(index).ok()?, i32::try_from(value).ok()?))
}

fn set_lua_integer_upvalue(lua: &Lua, function: &Function, index: i32, value: i32) -> bool {
    // SAFETY: see `lua_result_mode_upvalue`; Rust-side argument conversion also
    // guarantees the C callback receives a function and two integers.
    let Ok(helper) = (unsafe { lua.create_c_function(set_integer_upvalue) }) else {
        return false;
    };
    helper.call::<bool>((function.clone(), index, value)).unwrap_or(false)
}

fn lua_flag_score_upvalue(lua: &Lua, function: &Function) -> Option<(i32, bool)> {
    // SAFETY: the callback obeys Lua's C function ABI and accesses only its
    // call frame. It is retained by mlua for the duration of `call`.
    let helper = unsafe { lua.create_c_function(find_flag_score_upvalue).ok()? };
    let (index, value) = helper.call::<(i64, bool)>(function.clone()).ok()?;
    Some((i32::try_from(index).ok()?, value))
}

fn set_lua_boolean_upvalue(lua: &Lua, function: &Function, index: i32, value: bool) -> bool {
    // SAFETY: see `lua_flag_score_upvalue`; Rust-side argument conversion
    // guarantees the callback receives a function, integer, and boolean.
    let Ok(helper) = (unsafe { lua.create_c_function(set_boolean_upvalue) }) else {
        return false;
    };
    helper.call::<bool>((function.clone(), index, value)).unwrap_or(false)
}

fn postprocess_lua_skin_json(root: &mut JsonMap<String, JsonValue>, warnings: &mut Vec<String>) {
    repair_malformed_destination_ops(root, warnings);
    repair_select_score_rate_punctuation(root);
    let repaired = repair_keybeam_destination_draws(root);
    warnings.retain(|warning| {
        !repaired.iter().any(|index| {
            warning == &format!("skipping unsupported draw function at $.destination[{index}].draw")
                || warning
                    == &format!("skipping unsupported field `timer` at $.destination[{index}]")
        })
    });
}

/// Repairs two malformed `op` table shapes accepted by Lua/beatoraja skins but
/// rejected by the strict document schema. Keep the predicates narrow so an
/// unrelated object or intentionally nested array is not silently flattened.
fn repair_malformed_destination_ops(
    root: &mut JsonMap<String, JsonValue>,
    warnings: &mut Vec<String>,
) {
    let Some(destinations) = root.get_mut("destination").and_then(JsonValue::as_array_mut) else {
        return;
    };
    const DESTINATION_FIELDS: &[&str] = &[
        "blend",
        "filter",
        "timer",
        "timer_expr",
        "loop",
        "center",
        "offset",
        "offsets",
        "stretch",
        "draw",
        "dst",
        "mouseRect",
    ];
    let mut repaired_count = 0;

    for (index, destination) in destinations.iter_mut().enumerate() {
        let Some(destination) = destination.as_object_mut() else {
            continue;
        };
        let Some(op) = destination.remove("op") else {
            continue;
        };

        let repaired = match op {
            JsonValue::Object(mut mixed) => {
                let has_destination_marker = mixed.get("dst").is_some_and(JsonValue::is_array);
                let named_fields_are_known = mixed
                    .keys()
                    .filter(|key| key.parse::<usize>().is_err())
                    .all(|key| DESTINATION_FIELDS.contains(&key.as_str()));
                let named_fields_do_not_conflict = mixed
                    .keys()
                    .filter(|key| key.parse::<usize>().is_err())
                    .all(|key| !destination.contains_key(key));

                let mut numbered = mixed
                    .iter()
                    .filter_map(|(key, value)| {
                        key.parse::<usize>().ok().map(|position| (position, value.clone()))
                    })
                    .collect::<Vec<_>>();
                numbered.sort_by_key(|(position, _)| *position);
                let numbered_are_contiguous_i32 = !numbered.is_empty()
                    && numbered.iter().enumerate().all(|(offset, (position, value))| {
                        *position == offset + 1
                            && value.as_i64().and_then(|value| i32::try_from(value).ok()).is_some()
                    });

                if has_destination_marker
                    && named_fields_are_known
                    && named_fields_do_not_conflict
                    && numbered_are_contiguous_i32
                {
                    for key in mixed
                        .keys()
                        .filter(|key| key.parse::<usize>().is_err())
                        .cloned()
                        .collect::<Vec<_>>()
                    {
                        if let Some(value) = mixed.remove(&key) {
                            destination.insert(key, value);
                        }
                    }
                    destination.insert(
                        "op".to_string(),
                        JsonValue::Array(numbered.into_iter().map(|(_, value)| value).collect()),
                    );
                    warnings.retain(|warning| {
                        warning
                            != &format!(
                                "mixed lua table converted to object at $.destination[{}].op",
                                index + 1
                            )
                    });
                    true
                } else {
                    destination.insert("op".to_string(), JsonValue::Object(mixed));
                    false
                }
            }
            JsonValue::Array(mut outer) if outer.len() == 2 => {
                let head = outer.first().and_then(JsonValue::as_i64);
                let nested = outer.get(1).and_then(JsonValue::as_array);
                let nested_is_i32 = nested.is_some_and(|values| {
                    !values.is_empty()
                        && values.iter().all(|value| {
                            value.as_i64().and_then(|value| i32::try_from(value).ok()).is_some()
                        })
                });
                let redundant_prefix = head.is_some()
                    && nested.and_then(|values| values.first()).and_then(JsonValue::as_i64) == head;
                if nested_is_i32 && redundant_prefix {
                    destination.insert("op".to_string(), outer.swap_remove(1));
                    true
                } else {
                    destination.insert("op".to_string(), JsonValue::Array(outer));
                    false
                }
            }
            op => {
                destination.insert("op".to_string(), op);
                false
            }
        };

        if repaired {
            repaired_count += 1;
        }
    }
    if repaired_count > 0 {
        warnings.push(format!("repaired {repaired_count} malformed destination op tables"));
    }
}

fn skin_config_options_from_header(
    header: &JsonValue,
    selected: &BTreeMap<String, String>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, i64> {
    let mut result = BTreeMap::new();
    let Some(properties) = header.get("property").and_then(JsonValue::as_array) else {
        return result;
    };

    for property in properties {
        let Some(name) = property.get("name").and_then(JsonValue::as_str) else {
            continue;
        };
        let Some(items) = property.get("item").and_then(JsonValue::as_array) else {
            continue;
        };
        let selected_value = selected.get(name).map(String::as_str);
        let op = selected_value
            .and_then(|value| option_value_to_op(items, value))
            .or_else(|| default_property_op(property, items));
        if let Some(op) = op {
            result.insert(name.to_string(), op);
        } else {
            warnings.push(format!("property `{name}` has no selectable op"));
        }
    }

    for (key, value) in selected {
        if !result.contains_key(key) && value.parse::<i64>().is_err() {
            warnings.push(format!("option `{key}` did not match a skin property"));
        }
    }

    result
}

/// 無効な destination が Lua 評価時にも座標を要求するスキン向けの退避値。
/// property ごとに末尾の選択肢を採用し、通常の選択で初期化されなかった optional
/// layout を構築できるようにする。呼び出し元は描画用の有効 op を元選択で上書きする。
fn fallback_skin_config_options(
    header: &JsonValue,
    selected_options: &BTreeMap<String, i64>,
) -> BTreeMap<String, i64> {
    let mut fallback = selected_options.clone();
    let Some(properties) = header.get("property").and_then(JsonValue::as_array) else {
        return fallback;
    };

    for property in properties {
        let Some(name) = property.get("name").and_then(JsonValue::as_str) else {
            continue;
        };
        let Some(op) = property
            .get("item")
            .and_then(JsonValue::as_array)
            .and_then(|items| items.last())
            .and_then(|item| item.get("op"))
            .and_then(json_integer)
        else {
            continue;
        };
        fallback.insert(name.to_string(), op);
    }
    fallback
}

fn lua_nil_arithmetic_error(error: &mlua::Error) -> bool {
    error.to_string().contains("attempt to perform arithmetic on a nil value")
}

fn option_value_to_op(items: &[JsonValue], value: &str) -> Option<i64> {
    if let Ok(op) = value.parse::<i64>() {
        return items
            .iter()
            .find_map(|item| (item.get("op").and_then(json_integer) == Some(op)).then_some(op));
    }
    items.iter().find_map(|item| {
        (item.get("name").and_then(JsonValue::as_str) == Some(value))
            .then(|| item.get("op").and_then(json_integer))
            .flatten()
    })
}

fn default_property_op(property: &JsonValue, items: &[JsonValue]) -> Option<i64> {
    if let Some(default_name) = property.get("def").and_then(JsonValue::as_str)
        && let Some(op) = option_name_to_op(items, default_name)
    {
        return Some(op);
    }
    items.first().and_then(|item| item.get("op")).and_then(json_integer)
}

fn option_name_to_op(items: &[JsonValue], value: &str) -> Option<i64> {
    items.iter().find_map(|item| {
        (item.get("name").and_then(JsonValue::as_str) == Some(value))
            .then(|| item.get("op").and_then(json_integer))
            .flatten()
    })
}

fn json_integer(value: &JsonValue) -> Option<i64> {
    value.as_i64().or_else(|| {
        let value = value.as_f64()?;
        (value.is_finite()
            && value.fract() == 0.0
            && value >= i64::MIN as f64
            && value <= i64::MAX as f64)
            .then_some(value as i64)
    })
}

fn skin_config_offsets_from_header(
    header: &JsonValue,
    runtime_state: &LuaLoadRuntimeState,
) -> BTreeMap<String, LuaSkinOffsetValue> {
    let mut result = BTreeMap::new();
    for (name, id) in skin_offset_definitions_from_header(header) {
        result.insert(name.clone(), lua_skin_offset_value(runtime_state, &name, Some(id)));
    }
    result
}

fn skin_offset_definitions_from_header(header: &JsonValue) -> Vec<(String, i32)> {
    let mut result = Vec::new();
    if let Some(offsets) = header.get("offset").and_then(JsonValue::as_array) {
        for offset in offsets {
            let Some(name) = offset.get("name").and_then(JsonValue::as_str) else {
                continue;
            };
            let id = offset
                .get("id")
                .and_then(json_integer)
                .and_then(|id| i32::try_from(id).ok())
                .unwrap_or_default();
            result.push((name.to_string(), id));
        }
    }

    let skin_type = header.get("type").and_then(json_integer).and_then(|id| i32::try_from(id).ok());
    if matches!(skin_type, Some(0 | 1 | 2 | 3 | 4 | 12 | 13 | 16 | 17 | 21 | 22 | 23 | 24)) {
        // JSONSkinLoader appends these after custom definitions before
        // SkinLuaAccessor exports skin_config. BMZ offset 34 is intentionally
        // renderer-only and is not part of beatoraja's Lua configuration.
        for (name, id) in [
            ("All offset(%)", 10),
            ("Notes offset", 30),
            ("Judge offset", 32),
            ("Judge Detail offset", 33),
        ] {
            result.push((name.to_string(), id));
        }
    }

    result
}

fn lua_skin_offset_value(
    runtime_state: &LuaLoadRuntimeState,
    name: &str,
    id: Option<i32>,
) -> LuaSkinOffsetValue {
    runtime_state
        .offset_values
        .get(name)
        .copied()
        .or_else(|| id.and_then(|id| runtime_state.offset_id_values.get(&id).copied()))
        .unwrap_or_default()
}

/// スキン設定パネルで選んだファイル選択を、filepath 定義の `path` グロブごとに
/// 集める。キーは `path` グロブ (区切りを `/` に正規化)、値は選択ファイルの
/// スキンルート相対パス。選択が無い / 空の定義は含めない。
fn skin_files_from_header(
    root: &Path,
    header: &JsonValue,
    selected: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let Some(filepaths) = header.get("filepath").and_then(JsonValue::as_array) else {
        return result;
    };
    for filepath in filepaths {
        let Some(name) = filepath.get("name").and_then(JsonValue::as_str) else {
            continue;
        };
        let Some(path) = filepath.get("path").and_then(JsonValue::as_str) else {
            continue;
        };
        let normalized_path = path.replace('\\', "/");
        let choice = selected
            .get(name)
            .filter(|choice| !choice.is_empty())
            .cloned()
            .or_else(|| default_skin_file_from_filepath(root, &normalized_path, filepath));
        if let Some(choice) = choice {
            result.insert(normalized_path, choice);
        }
    }
    result
}

fn skin_named_files_from_header(
    root: &Path,
    header: &JsonValue,
    selected: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let Some(filepaths) = header.get("filepath").and_then(JsonValue::as_array) else {
        return result;
    };
    for filepath in filepaths {
        let Some(name) = filepath.get("name").and_then(JsonValue::as_str) else {
            continue;
        };
        let Some(path) = filepath.get("path").and_then(JsonValue::as_str) else {
            continue;
        };
        let normalized_path = path.replace('\\', "/");
        let choice = selected
            .get(name)
            .filter(|choice| !choice.is_empty())
            .cloned()
            .or_else(|| default_skin_file_from_filepath(root, &normalized_path, filepath));
        if let Some(choice) = choice {
            result.insert(name.to_string(), choice);
        }
    }
    result
}

fn skin_file_dependency_names_from_header(header: &JsonValue) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let Some(filepaths) = header.get("filepath").and_then(JsonValue::as_array) else {
        return result;
    };
    for filepath in filepaths {
        let Some(name) = filepath.get("name").and_then(JsonValue::as_str) else {
            continue;
        };
        let Some(path) = filepath.get("path").and_then(JsonValue::as_str) else {
            continue;
        };
        result.insert(path.replace('\\', "/"), name.to_string());
    }
    result
}

/// beatoraja のファイル選択カスタマイズで「ランダム」を表す番兵値。
/// `skin_files` の値がこれのとき、`skin_config.get_path` はロードごとに候補から
/// ランダムに選ぶ。
const RANDOM_FILE_SELECTION: &str = "Random";

/// `0..len` の範囲でロードごとに変わる擬似乱数インデックスを返す。
/// `RandomState` のプロセス内ランダムキーを使い、追加クレートなしで beatoraja
/// 相当の「毎ロードでランダム」を満たす。
fn random_skin_file_index(len: usize) -> usize {
    use std::hash::BuildHasher;

    debug_assert!(len > 0);
    let hash = std::collections::hash_map::RandomState::new().hash_one(len as u64);
    (hash % len as u64) as usize
}

fn default_skin_file_from_filepath(
    root: &Path,
    normalized_path: &str,
    filepath: &JsonValue,
) -> Option<String> {
    let candidates = skin_file_candidates(root, normalized_path);
    if candidates.is_empty() {
        return None;
    }
    let default_name = filepath.get("def").and_then(JsonValue::as_str).unwrap_or_default();
    if !default_name.is_empty() {
        // def="Random" は具体ファイルへ固定せず、ランダム番兵を既定にする。
        if default_name.eq_ignore_ascii_case(RANDOM_FILE_SELECTION) {
            return Some(RANDOM_FILE_SELECTION.to_string());
        }
        if let Some(candidate) =
            candidates.iter().find(|candidate| filename_matches_def(candidate, default_name))
        {
            return Some(candidate_file_name(candidate));
        }
    } else if let Some(candidate) =
        candidates.iter().find(|candidate| filename_matches_def(candidate, "default"))
    {
        return Some(candidate_file_name(candidate));
    }
    candidates.into_iter().next().map(|candidate| candidate_file_name(&candidate))
}

fn skin_file_candidates(root: &Path, normalized_path: &str) -> Vec<String> {
    let requested_path = strip_beatoraja_asset_filter(normalized_path);
    let Some((prefix, suffix)) = requested_path.split_once('*') else {
        return vec![requested_path.to_string()];
    };
    if suffix.contains('*') {
        return Vec::new();
    }
    let slash = prefix.rfind('/').map(|index| index + 1).unwrap_or(0);
    let (directory_prefix, name_prefix) = prefix.split_at(slash);
    let dir = root.join(directory_prefix);
    let mut candidates = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return candidates;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(name_prefix) {
            continue;
        }
        if let Some(nested_suffix) = suffix.strip_prefix('/') {
            let candidate = format!("{directory_prefix}{name}/{nested_suffix}");
            if root.join(&candidate).exists() {
                candidates.push(candidate);
            }
        } else if name.ends_with(suffix) {
            candidates.push(format!("{directory_prefix}{name}"));
        }
    }
    candidates.sort();
    candidates
}

fn filename_matches_def(candidate: &str, default_name: &str) -> bool {
    let file_name = Path::new(candidate).file_name().and_then(|name| name.to_str()).unwrap_or("");
    if file_name.eq_ignore_ascii_case(default_name) {
        return true;
    }
    let stem = Path::new(file_name).file_stem().and_then(|stem| stem.to_str()).unwrap_or(file_name);
    if stem.eq_ignore_ascii_case(default_name) {
        return true;
    }
    filepath_def_acronym(default_name).is_some_and(|acronym| {
        let stem_lower = stem.to_ascii_lowercase();
        let acronym_lower = acronym.to_ascii_lowercase();
        stem_lower == acronym_lower || stem_lower.starts_with(&acronym_lower)
    })
}

fn filepath_def_acronym(default_name: &str) -> Option<String> {
    if !default_name.contains('-') {
        return None;
    }
    let acronym = default_name
        .split('-')
        .filter_map(|part| part.chars().find(|ch| ch.is_ascii_alphanumeric()))
        .collect::<String>();
    (!acronym.is_empty()).then_some(acronym)
}

fn candidate_file_name(candidate: &str) -> String {
    Path::new(candidate).file_name().and_then(|name| name.to_str()).unwrap_or(candidate).to_string()
}

/// ユーザ選択のスキンルート相対パスを解決する。
///
/// 絶対パスやスキンルート外への脱出を含む選択は無効として `None` を返す。
/// 通常の候補解決経路 (`skin_config_get_path` 本体) と挙動を揃え、
/// ファイル / ディレクトリの双方を許可する (Lua スキンは
/// `skin_config.get_path("dir/*") .. "/foo.lua"` の形でディレクトリ選択を
/// 連結に使うパターンがある)。
fn resolve_selected_skin_path(root: &Path, selected: &str) -> Option<PathBuf> {
    let relative = Path::new(selected);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
        })
    {
        return None;
    }
    let candidate = root.join(relative);
    candidate.exists().then_some(candidate)
}

fn skin_config_get_path(
    root: &Path,
    requested: &str,
    skin_files: &BTreeMap<String, String>,
) -> Result<PathBuf> {
    let requested_path = strip_beatoraja_asset_filter(requested);
    let relative_path = Path::new(requested_path);
    if relative_path.is_absolute()
        || relative_path.components().any(|component| {
            matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
        })
    {
        bail!("skin_config.get_path escapes skin root: {requested}");
    }

    // ユーザがスキン設定パネルで「ランダム」を選んだときは、候補からロードごとに
    // ランダムに選ぶ (beatoraja のファイル選択 "Random" 相当)。
    let want_random =
        skin_files.get(&requested.replace('\\', "/")).is_some_and(|s| s == RANDOM_FILE_SELECTION);

    // ユーザがスキン設定パネルで選んだファイルを最優先で返す。
    // 選択が存在しない / ファイルが消えている場合は従来通り候補解決へ委ねる。
    if !want_random {
        if let Some(selected) = skin_files.get(&requested.replace('\\', "/"))
            && let Some(path) =
                resolve_selected_skin_path_for_pattern(root, requested_path, selected)
        {
            return Ok(path);
        }
        if let Some(path) =
            resolve_selected_skin_path_for_wildcard_child(root, requested_path, skin_files)
        {
            return Ok(path);
        }
    }

    let Some((prefix, suffix)) = requested_path.split_once('*') else {
        return Ok(root.join(requested_path));
    };
    if suffix.contains('*') {
        bail!("skin_config.get_path supports only one wildcard: {requested}");
    }

    let slash = prefix.rfind(['/', '\\']).map(|index| index + 1).unwrap_or(0);
    let (directory_prefix, name_prefix) = prefix.split_at(slash);
    let dir = root.join(directory_prefix);
    let suffix = suffix.replace('\\', "/");
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&dir)
        .with_context(|| format!("failed to read skin_config path dir: {}", dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(name_prefix) {
            continue;
        }
        let candidate_relative = if let Some(nested_suffix) = suffix.strip_prefix('/') {
            format!("{directory_prefix}{name}/{nested_suffix}")
        } else {
            if !name.ends_with(&suffix) {
                continue;
            }
            format!("{directory_prefix}{name}")
        };
        let candidate = root.join(candidate_relative);
        if candidate.exists() {
            candidates.push(candidate);
        }
    }
    if candidates.is_empty() {
        bail!("skin_config path not found: {requested}");
    }
    let index = if want_random { random_skin_file_index(candidates.len()) } else { 0 };
    Ok(candidates.swap_remove(index))
}

fn resolve_selected_skin_path_for_wildcard_child(
    root: &Path,
    requested: &str,
    skin_files: &BTreeMap<String, String>,
) -> Option<PathBuf> {
    let (requested_prefix, requested_suffix) = requested.split_once('*')?;
    for (configured, selected) in skin_files {
        let (configured_prefix, configured_suffix) = configured.split_once('*')?;
        if requested_prefix != configured_prefix {
            continue;
        }
        let wildcard = wildcard_from_selection(configured_prefix, configured_suffix, selected)?;
        let candidate = format!("{requested_prefix}{wildcard}{requested_suffix}");
        if let Some(path) = resolve_selected_skin_path(root, &candidate) {
            return Some(path);
        }
    }
    None
}

fn resolve_selected_skin_path_for_pattern(
    root: &Path,
    pattern: &str,
    selected: &str,
) -> Option<PathBuf> {
    if let Some(path) = resolve_selected_skin_path(root, selected) {
        return Some(path);
    }
    let pattern = strip_beatoraja_asset_filter(pattern).replace('\\', "/");
    let star = pattern.find('*')?;
    let prefix = &pattern[..star];
    let slash = prefix.rfind(['/', '\\']).map(|index| index + 1).unwrap_or(0);
    let directory_prefix = &prefix[..slash];
    resolve_selected_skin_path(root, &format!("{directory_prefix}{}", selected.replace('\\', "/")))
}

fn wildcard_from_selection<'a>(
    configured_prefix: &str,
    configured_suffix: &str,
    selected: &'a str,
) -> Option<&'a str> {
    selected
        .strip_prefix(configured_prefix)
        .and_then(|rest| rest.strip_suffix(configured_suffix).or(Some(rest)))
        .or_else(|| {
            let name_prefix = configured_prefix.rsplit(['/', '\\']).next().unwrap_or_default();
            selected
                .strip_prefix(name_prefix)
                .and_then(|rest| rest.strip_suffix(configured_suffix).or(Some(rest)))
        })
}

fn strip_beatoraja_asset_filter(path: &str) -> &str {
    path.split_once('|').map_or(path, |(asset_path, _)| asset_path)
}

/// `Path::canonicalize` returns Windows extended-length (`\\?\`) verbatim paths.
/// Verbatim paths reject `/` as a separator, but beatoraja Lua skins build paths
/// by string concatenation (e.g. `skin_config.get_path("_font/*") .. "/set.lua"`),
/// so a verbatim sandbox root makes every such `dofile`/`require` fail with a
/// path-syntax error. Strip the verbatim prefix so derived paths stay normal and
/// tolerate mixed separators. No-op on non-Windows.
fn canonicalize_skin_path(path: &Path) -> std::io::Result<PathBuf> {
    path.canonicalize().map(simplify_verbatim_path)
}

#[cfg(windows)]
fn simplify_verbatim_path(path: PathBuf) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        // Only simplify regular drive paths like `C:\dir`; leave device paths alone.
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return PathBuf::from(rest);
        }
    }
    path
}

#[cfg(not(windows))]
fn simplify_verbatim_path(path: PathBuf) -> PathBuf {
    path
}

fn resolve_lua_path(root: &Path, requested: &str, module: bool) -> Result<PathBuf> {
    let relative = if module { requested.replace('.', "/") } else { requested.to_string() };
    let relative_path = Path::new(&relative);
    if relative_path.is_absolute() {
        let canonical = canonicalize_skin_path(relative_path)?;
        if canonical.starts_with(root) {
            return Ok(canonical);
        }
        bail!("lua path escapes skin root: {requested}");
    }
    if relative_path.components().any(|component| {
        matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))
    }) {
        bail!("lua path escapes skin root: {requested}");
    }
    let candidates = if module {
        vec![format!("{relative}.lua"), format!("{relative}/init.lua")]
    } else if relative.ends_with(".lua") || relative.ends_with(".luaskin") {
        vec![relative]
    } else {
        vec![relative.clone(), format!("{relative}.lua")]
    };

    for candidate in candidates {
        if let Some(path) = resolve_beatoraja_skin_alias(root, &candidate) {
            return Ok(path);
        }
        let path = root.join(candidate);
        if path.is_file() {
            let canonical = canonicalize_skin_path(&path)?;
            if !canonical.starts_with(root) {
                bail!("lua path escapes skin root: {}", canonical.display());
            }
            return Ok(canonical);
        }
    }

    bail!("lua file not found: {requested}");
}

fn resolve_skin_io_path(root: &Path, requested: &str) -> Result<PathBuf> {
    let relative = normalize_skin_io_relative_path(requested)?;

    if let Some(path) = resolve_beatoraja_skin_alias(root, &relative) {
        return Ok(path);
    }

    let path = root.join(&relative);
    let canonical = canonicalize_skin_path(&path)?;
    if !canonical.starts_with(root) {
        bail!("io path escapes skin root: {}", canonical.display());
    }
    Ok(canonical)
}

fn normalize_skin_io_relative_path(requested: &str) -> Result<String> {
    if requested.contains('\0') {
        bail!("io path contains NUL");
    }
    let relative = requested.replace('\\', "/");
    if relative.starts_with('/') || relative.starts_with("//") {
        bail!("io path escapes skin root: {requested}");
    }
    let mut normalized = Vec::new();
    for (index, component) in relative.split('/').enumerate() {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".."
            || (index == 0
                && component.as_bytes().get(1) == Some(&b':')
                && component.as_bytes().first().is_some_and(u8::is_ascii_alphabetic))
        {
            bail!("io path escapes skin root: {requested}");
        }
        normalized.push(component);
    }
    if normalized.is_empty() {
        bail!("io path is empty");
    }
    Ok(normalized.join("/"))
}

fn normalize_virtual_io_files(
    files: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut normalized = BTreeMap::new();
    for (path, source) in files {
        let path = normalize_skin_io_relative_path(path)
            .with_context(|| format!("invalid Lua virtual IO path: {path}"))?;
        if source.len() > LUA_IO_MAX_READ_BYTES {
            bail!("Lua virtual IO file exceeds {} byte limit: {path}", LUA_IO_MAX_READ_BYTES);
        }
        if normalized.insert(path.clone(), source.clone()).is_some() {
            bail!("duplicate normalized Lua virtual IO path: {path}");
        }
    }
    Ok(normalized)
}

fn read_skin_io_source(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > LUA_IO_MAX_READ_BYTES as u64 {
        bail!("Lua IO file exceeds {} byte limit: {}", LUA_IO_MAX_READ_BYTES, path.display());
    }
    let source = fs::read_to_string(path)?;
    if source.len() > LUA_IO_MAX_READ_BYTES {
        bail!("Lua IO file exceeds {} byte limit: {}", LUA_IO_MAX_READ_BYTES, path.display());
    }
    Ok(source)
}

fn record_virtual_io_dependency(
    path: &str,
    source: Option<&str>,
    dependencies: Option<&Arc<Mutex<SkinLoadDependencies>>>,
) {
    if let Some(dependencies) = dependencies
        && let Ok(mut dependencies) = dependencies.lock()
    {
        dependencies.virtual_io_files.insert(path.to_string(), source.map(str::to_string));
    }
}

fn mark_io_dependency_opaque(dependencies: Option<&Arc<Mutex<SkinLoadDependencies>>>) {
    if let Some(dependencies) = dependencies
        && let Ok(mut dependencies) = dependencies.lock()
    {
        // A missing real file cannot be represented by loaded_files metadata.
        // Avoid caching a branch that could change merely because the file is
        // created after this load.
        dependencies.opaque = true;
    }
}

fn resolve_beatoraja_skin_alias(root: &Path, relative: &str) -> Option<PathBuf> {
    let rest = relative.strip_prefix("skin/")?;
    let (skin_name, skin_relative) = rest.split_once('/')?;
    if let Some(canonical) = canonicalize_skin_child(root, skin_relative) {
        return Some(canonical);
    }
    for ancestor in root.ancestors() {
        if ancestor.file_name().and_then(|name| name.to_str()) != Some(skin_name) {
            continue;
        }
        if let Some(canonical) = canonicalize_skin_child(ancestor, skin_relative) {
            return Some(canonical);
        }
    }
    None
}

fn canonicalize_skin_child(root: &Path, relative: &str) -> Option<PathBuf> {
    let path = root.join(relative);
    if !path.is_file() {
        return None;
    }
    let Ok(root) = canonicalize_skin_path(root) else {
        return None;
    };
    let Ok(canonical) = canonicalize_skin_path(&path) else {
        return None;
    };
    canonical.starts_with(&root).then_some(canonical)
}

fn is_unsupported_json_field_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Function(_)
            | Value::Thread(_)
            | Value::UserData(_)
            | Value::LightUserData(_)
            | Value::Error(_)
            | Value::Other(_)
    )
}

/// beatoraja Lua skin loader が document/header に残すコールバック。
/// BMZ は `.luaskin` 実行結果だけを使い、関数参照自体は JSON 化しない。
const SILENTLY_SKIPPED_LOADER_FIELDS: &[&str] = &["process", "main", "processHeader", "act"];

fn should_silently_skip_loader_field(key: &str, value: &Value) -> bool {
    matches!(value, Value::Function(_)) && SILENTLY_SKIPPED_LOADER_FIELDS.contains(&key)
}

fn lua_key_to_json_key(key: Value, path: &str, warnings: &mut Vec<String>) -> Result<String> {
    match key {
        Value::String(value) => Ok(value.to_string_lossy()),
        Value::Integer(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Boolean(value) => Ok(value.to_string()),
        _ => {
            warnings.push(format!("unsupported table key converted with debug fallback at {path}"));
            Ok(lua_value_to_log_string(&key))
        }
    }
}

#[cfg(test)]
mod tests {
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
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
            infer_result_panel_act_at_load(
                &lua,
                &functions.get::<Function>("ir_act").unwrap(),
                &probe,
            ),
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
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/MILLIONDOLLAR/result.luaskin");
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
        let messages: Vec<_> =
            loaded.warnings.iter().map(|warning| warning.message.as_str()).collect();
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
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
        lua.globals()
            .set("main_state", create_main_state_stub(&lua, probe.clone()).unwrap())
            .unwrap();
        let function = lua
            .load("return function() return main_state.timer(150) end")
            .eval::<Function>()
            .unwrap();

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
        assert_eq!(
            infer_boolean_predicate(&draw, &probe, None),
            Some("number(112) > 0".to_string())
        );
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
            serde_json::from_str(r#"{ "name": "BG", "path": "bg/*.mp4", "def": "Random" }"#)
                .unwrap();

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
        let skin_files =
            BTreeMap::from([("bg/*.mp4".to_string(), RANDOM_FILE_SELECTION.to_string())]);

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
}
