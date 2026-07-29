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
#[path = "lua/sandbox/budget.rs"]
mod sandbox_budget;
#[path = "lua/sandbox/environment.rs"]
mod sandbox_environment;
#[path = "lua/sandbox/event.rs"]
mod sandbox_event;
#[path = "lua/sandbox/io.rs"]
mod sandbox_io;
#[path = "lua/sandbox/main_state.rs"]
mod sandbox_main_state;
#[path = "lua/sandbox/probe.rs"]
mod sandbox_probe;

use conversion::*;
use function_inference::*;
use sandbox_budget::*;
use sandbox_environment::*;
use sandbox_event::*;
use sandbox_io::*;
use sandbox_main_state::*;
use sandbox_probe::*;

const LUA_INSTRUCTION_LIMIT: i64 = 2_000_000;
const LUA_INFERENCE_INSTRUCTION_LIMIT: i64 = 16_000_000;
const LUA_HOOK_INTERVAL: u32 = 1_000;
const LUA_MAX_TABLE_DEPTH: usize = 64;
const LUA_MAX_TABLE_ENTRIES: usize = 200_000;
const LUA_IO_MAX_READ_BYTES: usize = 8 * 1024 * 1024;
const LUA_MEMORY_LIMIT_BYTES: usize = 256 * 1024 * 1024;
const TIMER_OFF_VALUE: i32 = i32::MIN;
pub const LUA_DRAW_CALLBACK_PREFIX: &str = "bmz:lua_draw_callback:";

include!("lua/runtime.rs");
include!("lua/loading.rs");
include!("lua/execution.rs");
include!("lua/postprocess.rs");
include!("lua/config.rs");
include!("lua/paths.rs");

#[cfg(test)]
#[path = "lua/tests.rs"]
mod tests;
