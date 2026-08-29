use super::*;

#[derive(Debug)]
pub(super) struct TimerObserveState {
    pub(super) timer_value: i32,
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
                    mark_load_dependency_opaque(dependencies_for_open.as_ref());
                    return Ok(Value::Nil);
                };
                let Ok(source) = read_skin_io_source(&path) else {
                    mark_load_dependency_opaque(dependencies_for_open.as_ref());
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
                    mark_load_dependency_opaque(dependencies_for_lines.as_ref());
                    return create_lines_iterator(
                        lua,
                        Arc::new(Mutex::new(ReadFileState::default())),
                    );
                };
                let Ok(source) = read_skin_io_source(&path) else {
                    mark_load_dependency_opaque(dependencies_for_lines.as_ref());
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
    pub(super) year: i32,
    pub(super) month: u32,
    pub(super) day: u32,
    pub(super) hour: u32,
    pub(super) minute: u32,
    pub(super) second: u32,
    pub(super) weekday: u32,
    pub(super) yearday: u32,
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
