pub(in crate::ui) struct SkinUiPathContext {
    root: PathBuf,
    package: Option<bmz_skin::SkinPathContext>,
}

impl SkinUiPathContext {
    #[cfg(test)]
    pub(in crate::ui) fn legacy(root: &Path) -> Self {
        Self { root: root.to_path_buf(), package: None }
    }

    #[cfg(test)]
    pub(in crate::ui) fn package(root: &Path, package: bmz_skin::SkinPathContext) -> Self {
        Self { root: root.to_path_buf(), package: Some(package) }
    }
}

/// スキンパス文字列から設定 UI 用のパス context を得る。
pub(in crate::ui) fn skin_root_path(
    app_paths: &AppPaths,
    skin_path: &str,
) -> Option<SkinUiPathContext> {
    let trimmed = skin_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = app_paths.resolve_path_ref(trimmed).ok()?;
    let root = if path.is_dir() { path.clone() } else { path.parent()?.to_path_buf() };
    let package = if is_lua_skin_path(&path) {
        bmz_skin::SkinPathContext::new(&path, app_paths.skin_library_roots()).ok()
    } else {
        None
    };
    Some(SkinUiPathContext { root, package })
}

pub(in crate::ui) fn glob_candidates_for_skin(
    context: &SkinUiPathContext,
    pattern: &str,
) -> Vec<String> {
    match &context.package {
        Some(package) => package.wildcard_candidate_values(pattern).unwrap_or_default(),
        None => glob_candidates(&context.root, pattern),
    }
}

/// `pattern` (スキンルート相対、末尾要素にワイルドカード `*` を 1 個まで) に
/// マッチするファイルの相対パス一覧を返す。
///
/// beatoraja の `path|filter|` 形式の `|...|` 接尾辞 (lanecover などの
/// アセット用途タグ) は対象ファイル名には含まれないので、列挙前に取り除く。
pub(in crate::ui) fn glob_candidates(root: &Path, pattern: &str) -> Vec<String> {
    let pattern = pattern.replace('\\', "/");
    let pattern = pattern.split_once('|').map_or(pattern.as_str(), |(path, _)| path).to_string();
    let (dir_part, name_part) = match pattern.rfind('/') {
        Some(index) => (&pattern[..=index], &pattern[index + 1..]),
        None => ("", pattern.as_str()),
    };
    let Some((prefix, suffix)) = name_part.split_once('*') else {
        // ワイルドカード無し: パターンそのものを唯一の候補とする。
        return vec![pattern.clone()];
    };
    let dir = root.join(dir_part);
    let mut candidates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
            {
                candidates.push(format!("{dir_part}{name}"));
            }
        }
    }
    candidates.sort();
    candidates
}

pub(in crate::ui) fn normalize_filepath_selection(
    selected: &str,
    candidates: &[String],
) -> Option<String> {
    if selected.is_empty() || selected == RANDOM_FILE_SELECTION {
        return None;
    }
    let normalized = selected.replace('\\', "/");
    if candidates.iter().any(|candidate| candidate == &normalized) {
        return (normalized != selected).then_some(normalized);
    }
    if normalized.contains('/') {
        return None;
    }
    candidates
        .iter()
        .find(|candidate| {
            filepath_selection_label(candidate).eq_ignore_ascii_case(normalized.as_str())
        })
        .cloned()
}

pub(in crate::ui) fn filepath_selection_label(value: &str) -> &str {
    let slash = value.rfind('/').into_iter().chain(value.rfind('\\')).max();
    match slash {
        Some(index) if index + 1 < value.len() => &value[index + 1..],
        _ => value,
    }
}

/// property の既定選択肢名。beatoraja と同じく `def` が item name と一致する
/// ときだけ採用し、未指定/不一致なら先頭 item を使う。
pub(in crate::ui) fn property_default(prop: &SkinPropertyDef) -> String {
    prop.item
        .iter()
        .find(|item| !prop.def.is_empty() && item.name == prop.def)
        .or_else(|| prop.item.first())
        .map(|item| item.name.clone())
        .unwrap_or_default()
}

pub(in crate::ui) fn property_selection_is_valid(prop: &SkinPropertyDef, selected: &str) -> bool {
    if let Ok(op) = selected.parse::<i32>() {
        return prop.item.iter().any(|item| item.op == op);
    }
    prop.item.iter().any(|item| item.name == selected)
}

pub(in crate::ui) fn filepath_default(
    filepath: &SkinFilepathDef,
    candidates: &[String],
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    // def が "Random" のときは具体ファイルへ固定せず、ランダム番兵を既定にする
    // (beatoraja の def="Random" 相当)。
    if filepath.def.eq_ignore_ascii_case(RANDOM_FILE_SELECTION) {
        return Some(RANDOM_FILE_SELECTION.to_string());
    }
    if !filepath.def.is_empty()
        && let Some(candidate) =
            candidates.iter().find(|candidate| filename_matches_def(candidate, &filepath.def))
    {
        return Some(candidate.clone());
    }
    if filepath.def.is_empty()
        && let Some(candidate) =
            candidates.iter().find(|candidate| filename_matches_def(candidate, "default"))
    {
        return Some(candidate.clone());
    }
    candidates.first().cloned()
}

pub(in crate::ui) fn filename_matches_def(candidate: &str, def: &str) -> bool {
    let file_name = Path::new(candidate).file_name().and_then(|name| name.to_str()).unwrap_or("");
    if file_name.eq_ignore_ascii_case(def) {
        return true;
    }
    let stem = Path::new(file_name).file_stem().and_then(|stem| stem.to_str()).unwrap_or(file_name);
    if stem.eq_ignore_ascii_case(def) {
        return true;
    }
    filepath_def_acronym(def).is_some_and(|acronym| {
        let stem_lower = stem.to_ascii_lowercase();
        let acronym_lower = acronym.to_ascii_lowercase();
        stem_lower == acronym_lower || stem_lower.starts_with(&acronym_lower)
    })
}

pub(in crate::ui) fn filepath_def_acronym(def: &str) -> Option<String> {
    if !def.contains('-') {
        return None;
    }
    let acronym = def
        .split('-')
        .filter_map(|part| part.chars().find(|ch| ch.is_ascii_alphanumeric()))
        .collect::<String>();
    (!acronym.is_empty()).then_some(acronym)
}
use super::*;
