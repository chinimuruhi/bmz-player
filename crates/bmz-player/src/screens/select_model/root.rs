use super::*;

/// Returns folder items for the virtual root, one entry per enabled root path.
pub fn root_folder_items(root_paths: &[String]) -> Vec<SelectItem> {
    root_paths
        .iter()
        .map(|path| {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path.as_str())
                .to_string();
            SelectItem::Folder {
                path: path.clone(),
                name,
                kind: SelectRowKind::Folder,
                summary: None,
            }
        })
        .collect()
}

pub fn favorite_root_item() -> SelectItem {
    SelectItem::Folder {
        path: FAVORITE_ROOT_PATH.to_string(),
        name: "FAVORITE".to_string(),
        kind: SelectRowKind::TableFolder,
        summary: None,
    }
}

pub fn favorite_root_items(collection_db: &CollectionDatabase) -> Result<Vec<SelectItem>> {
    let mut items = Vec::new();
    if !collection_db.favorite_chart_records()?.is_empty() {
        items.push(SelectItem::Folder {
            path: FAVORITE_CHART_PATH.to_string(),
            name: "FAVORITE CHART".to_string(),
            kind: SelectRowKind::TableFolder,
            summary: None,
        });
    }
    if !collection_db.favorite_song_records()?.is_empty() {
        items.push(SelectItem::Folder {
            path: FAVORITE_SONG_PATH.to_string(),
            name: "FAVORITE SONG".to_string(),
            kind: SelectRowKind::TableFolder,
            summary: None,
        });
    }
    Ok(items)
}

pub fn random_select_item_from_items(items: &[SelectItem]) -> Option<SelectItem> {
    let mut chart_ids = Vec::new();
    for item in items {
        if let SelectItem::Chart(row) = item
            && let Some(chart) = &row.chart
        {
            chart_ids.push(chart.chart_id);
        }
    }
    (!chart_ids.is_empty()).then(|| {
        SelectItem::Executable(SelectExecutableRow {
            title: "RANDOM SELECT".to_string(),
            kind: SelectExecutableKind::RandomSelect,
            chart_ids,
        })
    })
}

pub fn random_mix_item() -> SelectItem {
    SelectItem::Executable(SelectExecutableRow {
        title: "RANDOM MIX".to_string(),
        kind: SelectExecutableKind::RandomMix,
        chart_ids: Vec::new(),
    })
}
