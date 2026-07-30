use super::*;

pub(super) fn skin_path_options_need_full_reload(path: &Path) -> Result<bool> {
    if is_lua_skin_path(path) || is_lr2_skin_path(path) {
        return Ok(true);
    }
    if !is_json_skin_path(path) {
        return Ok(true);
    }
    json_skin_has_load_time_option_expansion(path)
}

pub(super) fn json_skin_has_load_time_option_expansion(path: &Path) -> Result<bool> {
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize skin root: {}", root.display()))?;
    let mut visited = HashSet::new();
    json_skin_file_has_load_time_option_expansion(path, &root, &mut visited)
}

pub(super) fn json_skin_file_has_load_time_option_expansion(
    path: &Path,
    root: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<bool> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize skin json: {}", path.display()))?;
    if !visited.insert(path.clone()) {
        return Ok(false);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read skin json: {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse skin json: {}", path.display()))?;
    let current_dir = path.parent().unwrap_or(root);
    json_skin_value_has_load_time_option_expansion(&value, current_dir, root, visited)
}

pub(super) fn json_skin_value_has_load_time_option_expansion(
    value: &serde_json::Value,
    current_dir: &Path,
    root: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<bool> {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                if json_skin_value_has_load_time_option_expansion(item, current_dir, root, visited)?
                {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        serde_json::Value::Object(object) => {
            if let Some(include) = object.get("include") {
                let include = include.as_str().with_context(|| {
                    format!("skin json include must be a string in {}", current_dir.display())
                })?;
                let included = current_dir
                    .join(include)
                    .canonicalize()
                    .with_context(|| format!("failed to canonicalize skin include: {include}"))?;
                anyhow::ensure!(
                    included.starts_with(root),
                    "skin include escapes skin root: {}",
                    included.display()
                );
                if json_skin_file_has_load_time_option_expansion(&included, root, visited)? {
                    return Ok(true);
                }
            }
            if object.contains_key("if")
                && (object.contains_key("value") || object.contains_key("values"))
            {
                return Ok(true);
            }
            for child in object.values() {
                if json_skin_value_has_load_time_option_expansion(
                    child,
                    current_dir,
                    root,
                    visited,
                )? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}
