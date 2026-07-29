#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LuaRuntimeFlagProbe {
    pub(super) id: i32,
    pub(super) table: String,
    pub(super) field: String,
    pub(super) initial: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum LuaRuntimeScalar {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LuaAudioActionKindProbe {
    Play,
    Loop,
    Stop,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LuaAudioActionProbe {
    pub(super) action: LuaAudioActionKindProbe,
    pub(super) path: String,
    pub(super) volume: f64,
}

/// beatoraja fast/slow 判定カウント ref (graph 比率推論用)
pub(super) const FAST_SLOW_FAST_REFS: [i32; 6] = [410, 412, 414, 416, 418, 421];
pub(super) const FAST_SLOW_SLOW_REFS: [i32; 6] = [411, 413, 415, 417, 419, 422];

pub(super) fn main_state_judge_ref(index: i32) -> Option<i32> {
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

pub(super) struct LuaRuntimeCallback {
    pub(super) path: String,
    pub(super) key: Option<RegistryKey>,
}

/// A Lua-only sidecar that owns the runtime VM and every callback registry key.
///
/// The VM is intentionally not cloneable. Its callbacks are obtained by a second
/// load after inference has completed, so inference can never mutate runtime
/// closure state, module state, or the Lua random-number generator.
pub struct LuaSkinRuntime {
    pub(super) lua: Lua,
    pub(super) callbacks: Vec<LuaRuntimeCallback>,
    pub(super) main_state_key: RegistryKey,
    pub(super) instruction_budget: LuaInstructionBudget,
    pub(super) skin_path: PathBuf,
    pub(super) failed_callbacks: BTreeSet<usize>,
    pub(super) failure_log_count: usize,
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
use super::*;
