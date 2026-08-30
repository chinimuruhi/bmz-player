use super::*;

pub(super) fn prepare_pm_chara(
    document: &mut SkinDocument,
    skin_root: &Path,
    resolved_files: &BTreeMap<String, String>,
    path_context: Option<&SkinPathContext>,
) {
    let mut expanded_sources = Vec::new();
    for index in 0..document.pmchara.len() {
        let (id, source_id, chara_type, color) = {
            let definition = &document.pmchara[index];
            (definition.id.clone(), definition.src.clone(), definition.chara_type, definition.color)
        };
        let Some(source_path) = document
            .source
            .iter()
            .find(|source| source.id == source_id)
            .map(|source| source.path.clone())
        else {
            tracing::warn!(pmchara_id = %id, source_id = %source_id, "PMchara source id is missing");
            continue;
        };
        let Some(resolved) = resolve_pm_chara_source_path(
            skin_root,
            path_context,
            &source_path,
            document,
            resolved_files,
        ) else {
            tracing::warn!(pmchara_id = %id, source = %source_path, "PMchara source path is missing");
            continue;
        };
        let prefix = format!("__bmz_pmchara:{index}:{id}");
        match bmz_skin::load_pm_chara(&resolved, &prefix, chara_type, color) {
            Ok(loaded) => {
                document.pmchara[index].runtime = Some(loaded.runtime);
                expanded_sources.extend(loaded.sources);
            }
            Err(error) => {
                tracing::warn!(
                    pmchara_id = %id,
                    path = %resolved.display(),
                    %error,
                    "failed to expand PMchara source"
                );
            }
        }
    }
    document.source.extend(expanded_sources);
}

fn resolve_pm_chara_source_path(
    skin_root: &Path,
    path_context: Option<&SkinPathContext>,
    source_path: &str,
    document: &SkinDocument,
    resolved_files: &BTreeMap<String, String>,
) -> Option<PathBuf> {
    let normalized = source_path.replace('\\', "/");
    let Some(path_context) = path_context else {
        return resolve_json_pm_chara_path(skin_root, &normalized, document, resolved_files);
    };
    if !normalized.contains('*') {
        return path_context.resolve_path(&normalized).ok();
    }

    let filepath =
        document.filepath.iter().find(|filepath| filepath.path.replace('\\', "/") == normalized);
    let selected = filepath
        .and_then(|filepath| resolved_files.get(&filepath.name))
        .filter(|selected| !selected.is_empty() && selected.as_str() != RANDOM_FILE_SELECTION)
        .map(String::as_str)
        .or_else(|| {
            filepath
                .filter(|filepath| !filepath.def.is_empty() && filepath.def != "Random")
                .map(|filepath| filepath.def.as_str())
        });
    if let Some(selected) = selected
        && let Some(path) = path_context.resolve_selected_for_pattern(&normalized, selected)
    {
        return Some(path);
    }

    let candidates = path_context.wildcard_candidates(&normalized).ok()?;
    if let Some(selected) = selected
        && let Some(path) = candidates.iter().find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(selected))
        })
    {
        return Some(path.clone());
    }
    candidates.into_iter().next()
}

fn resolve_json_pm_chara_path(
    skin_root: &Path,
    pattern: &str,
    document: &SkinDocument,
    resolved_files: &BTreeMap<String, String>,
) -> Option<PathBuf> {
    let pattern = strip_beatoraja_asset_filter(pattern);
    if !pattern.contains('*') {
        let path = resolve_case_insensitive_path(&skin_root.join(pattern));
        return (path.is_dir() || path.is_file()).then_some(path);
    }

    let filepath =
        document.filepath.iter().find(|filepath| filepath.path.replace('\\', "/") == pattern);
    let selected = filepath
        .and_then(|filepath| resolved_files.get(&filepath.name))
        .filter(|selected| !selected.is_empty() && selected.as_str() != RANDOM_FILE_SELECTION)
        .map(String::as_str)
        .or_else(|| {
            filepath
                .filter(|filepath| !filepath.def.is_empty() && filepath.def != "Random")
                .map(|filepath| filepath.def.as_str())
        });

    let star = pattern.find('*')?;
    let (prefix, suffix_with_star) = pattern.split_at(star);
    let suffix = &suffix_with_star[1..];
    let slash = prefix.rfind('/').map(|index| index + 1).unwrap_or(0);
    let (directory, name_prefix) = prefix.split_at(slash);
    let directory = resolve_case_insensitive_path(&skin_root.join(directory));

    if let Some(selected) = selected {
        let selected = selected.replace('\\', "/");
        let direct = resolve_case_insensitive_path(&skin_root.join(&selected));
        if direct.is_dir() || direct.is_file() {
            return Some(direct);
        }
        let relative = resolve_case_insensitive_path(&directory.join(&selected));
        if relative.is_dir() || relative.is_file() {
            return Some(relative);
        }
    }

    let mut candidates = std::fs::read_dir(directory)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
                starts_with_ignore_ascii_case(name, name_prefix)
                    && (suffix.starts_with('/') || ends_with_ignore_ascii_case(name, suffix))
            })
        })
        .filter_map(|path| {
            if let Some(suffix) = suffix.strip_prefix('/') {
                let nested = resolve_case_insensitive_path(&path.join(suffix));
                (nested.is_dir() || nested.is_file()).then_some(nested)
            } else {
                (path.is_dir() || path.is_file()).then_some(path)
            }
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().next()
}
