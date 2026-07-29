use super::*;

#[test]
fn wmii_fhd_lr2skin_2p_side_maps_single_play_notes_to_active_lanes_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("PLAY SIDE".to_string(), "2P".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let note = decoded.document.note.as_ref().expect("WMII note definition should load");

    assert!(
        note.dst.len() <= 8,
        "single-play 2P side should remap LR2 2P lanes into active lanes; got {} dst lanes",
        note.dst.len()
    );
    assert!(
        note.dst.iter().take(8).any(|entry| match entry {
            bmz_render::skin::SkinDstEntry::Frame(frame) =>
                frame.w.unwrap_or_default() > 0 && frame.h.unwrap_or_default() > 0,
            bmz_render::skin::SkinDstEntry::Conditional { frames, .. } =>
                frames.iter().any(|frame| {
                    frame.w.unwrap_or_default() > 0 && frame.h.unwrap_or_default() > 0
                }),
        }),
        "expected remapped 2P note lanes to have visible destinations"
    );
}

#[test]
fn lr2_document_cache_reuses_when_unused_option_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-option");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.lr2skin");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMOPTION,Unused,930,Off,On
#CUSTOMOPTION,Branch,910,Off,On
#IF,911
#IMAGE,on.png
#ELSE
#IMAGE,off.png
#ENDIF
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "off.png");

    let unused_changed = BTreeMap::from([("Unused".to_string(), "On".to_string())]);
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &unused_changed,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Hit);
    assert_eq!(second.document.source[0].path, "off.png");
    assert!(second.document.enabled_options().contains(&931));

    let branch_changed = BTreeMap::from([("Branch".to_string(), "On".to_string())]);
    let third = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &branch_changed,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(third.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(third.document.source[0].path, "on.png");
}

#[test]
fn lr2_document_cache_misses_when_play_side_remap_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-play-side");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.lr2skin");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMOPTION,PLAY SIDE,900,1P,2P
#IMAGE,base.png
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "base.png");

    let play_side_2p = BTreeMap::from([("PLAY SIDE".to_string(), "2P".to_string())]);
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &play_side_2p,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "base.png");
}

#[test]
fn lr2_document_cache_misses_when_included_file_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-include");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.lr2skin");
    let include_path = root.join("parts.csv");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#INCLUDE,parts.csv
"#,
    )
    .unwrap();
    std::fs::write(&include_path, "#IMAGE,off.png\n").unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "off.png");

    std::fs::write(&include_path, "#IMAGE,on-longer-name.png\n").unwrap();
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "on-longer-name.png");
}

#[test]
fn lr2_document_cache_misses_when_used_file_selection_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-file");
    std::fs::create_dir_all(root.join("parts")).unwrap();
    std::fs::write(root.join("parts/blue.png"), []).unwrap();
    std::fs::write(root.join("parts/red.png"), []).unwrap();
    let skin_path = root.join("play.lr2skin");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMFILE,Parts,parts/*.png,blue
#IMAGE,parts/*.png
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "parts/blue.png");

    let selected = BTreeMap::from([("Parts".to_string(), "red.png".to_string())]);
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &selected,
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "parts/red.png");
}
