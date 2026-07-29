use super::*;

#[test]
fn lua_document_cache_reuses_when_unused_option_changes() {
    let root = unique_test_dir("bmz-lua-document-cache-option");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.luaskin");
    std::fs::write(
            &skin_path,
            r#"
local branch = 910
if skin_config and skin_config.option then
    branch = skin_config.option["Branch"] or 910
end
return {
    type = 0,
    property = {
        { name = "Unused", item = {{ name = "Off", op = 900 }, { name = "On", op = 901 }}, def = "Off" },
        { name = "Branch", item = {{ name = "Off", op = 910 }, { name = "On", op = 911 }}, def = "Off" },
    },
    source = {
        { id = "bg", path = branch == 911 and "on.png" or "off.png" },
    },
}
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
    assert!(second.document.enabled_options().contains(&901));

    let branch_changed = BTreeMap::from([("Branch".to_string(), "On".to_string())]);
    let third = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &branch_changed,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(third.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(third.document.source[0].path, "on.png");
}

#[test]
fn lua_document_cache_misses_when_required_module_option_changes() {
    let root = unique_test_dir("bmz-lua-document-cache-required-option");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.luaskin");
    let module_path = root.join("parts.lua");
    std::fs::write(
        &skin_path,
        r#"
local parts = require("parts")
return parts.build()
"#,
    )
    .unwrap();
    std::fs::write(
            &module_path,
            r#"
local M = {}
function M.build()
    local branch = 910
    if skin_config and skin_config.option then
        branch = skin_config.option["Branch"] or 910
    end
    return {
        type = 0,
        property = {
            { name = "Unused", item = {{ name = "Off", op = 900 }, { name = "On", op = 901 }}, def = "Off" },
            { name = "Branch", item = {{ name = "Off", op = 910 }, { name = "On", op = 911 }}, def = "Off" },
        },
        source = {
            { id = "bg", path = branch == 911 and "on.png" or "off.png" },
        },
    }
end
return M
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

    let branch_changed = BTreeMap::from([("Branch".to_string(), "On".to_string())]);
    let third = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &branch_changed,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(third.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(third.document.source[0].path, "on.png");
}

#[test]
fn lua_document_cache_misses_when_runtime_number_changes() {
    let root = unique_test_dir("bmz-lua-document-cache-number");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("result.luaskin");
    std::fs::write(
        &skin_path,
        r#"
local main_state = require("main_state")
local diff = main_state.number(178)
return {
    type = 7,
    source = {
        { id = "bg", path = diff == 0 and "zero.png" or "nonzero.png" },
    },
}
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let zero_state = LuaLoadRuntimeState {
        number_values: BTreeMap::from([(178, 0)]),
        text_values: BTreeMap::new(),
        option_values: BTreeMap::new(),
        ..LuaLoadRuntimeState::default()
    };
    let first = load_skin_document(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &zero_state,
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "zero.png");

    let nonzero_state = LuaLoadRuntimeState {
        number_values: BTreeMap::from([(178, -1)]),
        text_values: BTreeMap::new(),
        option_values: BTreeMap::new(),
        ..LuaLoadRuntimeState::default()
    };
    let second = load_skin_document(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &nonzero_state,
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "nonzero.png");
}

#[test]
fn lua_document_cache_misses_when_runtime_offset_changes() {
    let root = unique_test_dir("bmz-lua-document-cache-offset");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.luaskin");
    std::fs::write(
        &skin_path,
        r#"
local skin = {
    type = 1,
    offset = {
        { name = "Panel", id = 42, x = true },
    },
}
if skin_config == nil then
    return skin
end
local panel_x = skin_config.offset["Panel"].x
skin.source = {
    { id = "bg", path = panel_x == 0 and "zero.png" or "nonzero.png" },
}
return skin
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));
    let offset = |x| LuaLoadRuntimeState {
        offset_values: BTreeMap::from([(
            "Panel".to_string(),
            bmz_skin::LuaSkinOffsetValue { x, ..Default::default() },
        )]),
        offset_id_values: BTreeMap::from([(
            42,
            bmz_skin::LuaSkinOffsetValue { x, ..Default::default() },
        )]),
        ..Default::default()
    };

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &offset(0),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "zero.png");

    let same = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &offset(0),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(same.cache_status, DocumentCacheStatus::Hit);

    let changed = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &offset(12),
        Some(cache),
    )
    .unwrap();
    assert_eq!(changed.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(changed.document.source[0].path, "nonzero.png");
}

#[test]
fn lua_document_cache_misses_when_runtime_event_index_changes() {
    let root = unique_test_dir("bmz-lua-document-cache-event-index");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("result.luaskin");
    std::fs::write(
        &skin_path,
        r#"
local main_state = require("main_state")
local lnmode = main_state.event_index(308)
return {
    type = 7,
    source = {
        { id = "bg", path = lnmode == 0 and "ln.png" or "charge.png" },
    },
}
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState {
            event_index_values: BTreeMap::from([(308, 0)]),
            ..LuaLoadRuntimeState::default()
        },
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "ln.png");

    let second = load_skin_document(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState {
            event_index_values: BTreeMap::from([(308, 2)]),
            ..LuaLoadRuntimeState::default()
        },
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "charge.png");
}

#[test]
fn lua_document_cache_misses_when_runtime_text_changes() {
    let root = unique_test_dir("bmz-lua-document-cache-text");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("select.luaskin");
    std::fs::write(
        &skin_path,
        r#"
local main_state = require("main_state")
return {
    type = 0,
    text = {
        { id = "player", constantText = main_state.text(2) },
    },
}
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Select,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState {
            text_values: BTreeMap::from([(2, "Player One".to_string())]),
            ..LuaLoadRuntimeState::default()
        },
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.text[0].constant_text, "Player One");

    let second = load_skin_document(
        &skin_path,
        SkinKind::Select,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState {
            text_values: BTreeMap::from([(2, "Player Two".to_string())]),
            ..LuaLoadRuntimeState::default()
        },
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.text[0].constant_text, "Player Two");
}

#[test]
fn lua_document_cache_misses_when_used_file_selection_changes() {
    let root = unique_test_dir("bmz-lua-document-cache-file");
    std::fs::create_dir_all(root.join("parts")).unwrap();
    std::fs::write(root.join("parts/blue.png"), []).unwrap();
    std::fs::write(root.join("parts/red.png"), []).unwrap();
    let skin_path = root.join("play.luaskin");
    std::fs::write(
        &skin_path,
        r#"
local path = "parts/blue.png"
if skin_config and skin_config.get_path then
    path = skin_config.get_path("parts/*.png")
end
return {
    type = 0,
    filepath = {
        { name = "Parts", path = "parts/*.png", def = "blue" },
    },
    source = {
        { id = "bg", path = path },
    },
}
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
    assert_eq!(
        Path::new(&first.document.source[0].path).canonicalize().unwrap(),
        std::fs::canonicalize(root.join("parts/blue.png")).unwrap()
    );

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
    assert_eq!(
        Path::new(&second.document.source[0].path).canonicalize().unwrap(),
        std::fs::canonicalize(root.join("parts/red.png")).unwrap()
    );
}

#[test]
fn required_skin_sources_excludes_unused_images() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "source": [
                    { "id": 1, "path": "used.png" },
                    { "id": 2, "path": "unused.png" },
                    { "id": 3, "path": "lift.png" }
                ],
                "image": [
                    { "id": "used", "src": 1, "x": 0, "y": 0, "w": 8, "h": 8 },
                    { "id": "unused", "src": 2, "x": 0, "y": 0, "w": 8, "h": 8 }
                ],
                "liftCover": [
                    { "id": "lift", "src": 3, "x": 0, "y": 0, "w": 8, "h": 8 }
                ],
                "destination": [
                    { "id": "used", "dst": [{ "x": 0, "y": 0, "w": 8, "h": 8 }] },
                    { "id": "lift", "dst": [{ "x": 0, "y": 0, "w": 8, "h": 8 }] }
                ]
            }
            "#,
    )
    .unwrap();

    let required = required_skin_source_ids(&document);

    assert!(required.contains("1"));
    assert!(!required.contains("2"));
    assert!(required.contains("3"));
}

#[test]
fn supported_font_paths_include_vector_and_bitmap_fonts() {
    assert!(is_supported_font_path(Path::new("font.ttf")));
    assert!(is_supported_font_path(Path::new("font.OTF")));
    assert!(is_supported_font_path(Path::new("font.ttc")));
    assert!(is_supported_font_path(Path::new("font.fnt")));
    assert!(!is_supported_font_path(Path::new("font.png")));
    assert!(is_bitmap_font_path(Path::new("font.fnt")));
    assert!(!is_bitmap_font_path(Path::new("font.ttf")));
}

#[test]
fn skin_font_cache_hit_skips_loader() {
    let root = unique_test_dir("bmz-font-cache-hit");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("font.ttf");
    std::fs::write(&path, b"not a real font").unwrap();
    let key = skin_font_cache_key(&path).unwrap();
    let expected = vec![1, 2, 3, 4];
    let cache = Arc::new(Mutex::new(SkinFontCache::default()));
    cache.lock().unwrap().insert(key.clone(), DecodedFontData::Vector(expected.clone()));

    let (actual, status, actual_key) = decode_font_with_cache(&path, Some(&cache)).unwrap();

    assert_eq!(status, FontCacheStatus::Hit);
    assert_eq!(actual_key, Some(key));
    match actual {
        DecodedFontData::Vector(bytes) => assert_eq!(bytes, expected),
        DecodedFontData::Bitmap(_) => panic!("expected cached vector font bytes"),
    }
}

#[test]
fn skin_font_cache_evicts_least_recently_used_entry() {
    let mut cache = SkinFontCache::with_limit_bytes(8);
    let a = test_font_cache_key("a.ttf");
    let b = test_font_cache_key("b.ttf");
    let c = test_font_cache_key("c.ttf");

    cache.insert(a.clone(), DecodedFontData::Vector(vec![1, 1, 1, 1]));
    cache.insert(b.clone(), DecodedFontData::Vector(vec![2, 2, 2, 2]));
    assert!(cache.get(&a).is_some());
    cache.insert(c.clone(), DecodedFontData::Vector(vec![3, 3, 3, 3]));

    assert!(cache.get(&a).is_some());
    assert!(cache.get(&b).is_none());
    assert!(cache.get(&c).is_some());
}

#[test]
fn skin_font_cache_skips_entries_larger_than_limit() {
    let mut cache = SkinFontCache::with_limit_bytes(4);
    let key = test_font_cache_key("too-large.ttf");

    cache.insert(key.clone(), DecodedFontData::Vector(vec![1, 2, 3, 4, 5]));

    assert!(cache.get(&key).is_none());
    assert_eq!(cache.total_bytes, 0);
}

#[test]
fn installed_font_snapshot_skips_font_payload_decode() {
    let root = unique_test_dir("bmz-installed-font-skip");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("skin.json");
    let font_path = root.join("font.ttf");
    std::fs::write(&font_path, b"not a real font").unwrap();
    std::fs::write(
        &skin_path,
        r#"
            {
                "type": 0,
                "font": [
                    { "id": "font1", "path": "font.ttf" }
                ]
            }
            "#,
    )
    .unwrap();
    let key = skin_font_cache_key(&font_path).unwrap();
    let installed = HashMap::from([("play:font1".to_string(), key.clone())]);

    let decoded = decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        None,
        None,
        None,
        None,
        Some(installed),
    )
    .unwrap();

    assert_eq!(decoded.stats.font_count, 1);
    assert_eq!(decoded.stats.font_payload_skipped, 1);
    assert_eq!(decoded.stats.font_cache_hits, 0);
    assert_eq!(decoded.stats.font_cache_misses, 0);
    assert_eq!(decoded.fonts.len(), 1);
    assert_eq!(decoded.fonts[0].stored_id, "play:font1");
    assert_eq!(decoded.fonts[0].cache_key.as_ref(), Some(&key));
    assert!(decoded.fonts[0].data.is_none());
}

#[test]
fn skin_source_asset_cache_hit_skips_loader() {
    let root = unique_test_dir("bmz-source-cache-hit");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("source.png");
    std::fs::write(&path, b"cached").unwrap();
    let key = skin_source_asset_cache_key(&path, false).unwrap();
    let expected = RgbaImageAsset { width: 1, height: 1, pixels: vec![1, 2, 3, 4] };
    let cache = Arc::new(Mutex::new(SkinSourceAssetCache::default()));
    cache.lock().unwrap().insert(key, expected.clone());

    let (actual, status) = load_source_asset_with_cache(&path, false, Some(&cache), || {
        panic!("cache hit must not call source loader")
    })
    .unwrap();

    assert_eq!(actual, expected);
    assert_eq!(status, SourceCacheStatus::Hit);
}

#[test]
fn skin_source_asset_cache_misses_after_metadata_change() {
    let root = unique_test_dir("bmz-source-cache-metadata");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("source.png");
    std::fs::write(&path, b"old").unwrap();
    let key = skin_source_asset_cache_key(&path, false).unwrap();
    let stale = RgbaImageAsset { width: 1, height: 1, pixels: vec![1, 2, 3, 4] };
    let fresh = RgbaImageAsset { width: 1, height: 1, pixels: vec![5, 6, 7, 8] };
    let cache = Arc::new(Mutex::new(SkinSourceAssetCache::default()));
    cache.lock().unwrap().insert(key, stale);

    std::fs::write(&path, b"new and longer").unwrap();
    let (actual, status) =
        load_source_asset_with_cache(&path, false, Some(&cache), || Ok(fresh.clone())).unwrap();

    assert_eq!(actual, fresh);
    assert_eq!(status, SourceCacheStatus::Miss);
}

#[test]
fn skin_gpu_texture_cache_reuses_inserted_source_textures() {
    let root = unique_test_dir("bmz-gpu-texture-cache");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("source.png");
    std::fs::write(&path, b"cached").unwrap();
    let key = skin_source_asset_cache_key(&path, false).unwrap();
    let size = SkinImageSize { width: 64.0, height: 32.0 };
    let mut cache = SkinGpuTextureCache::default();

    let allocated = cache.allocate_texture_id(SkinKind::Play);
    cache.insert(key.clone(), allocated, size);

    let cached = cache.get(&key).unwrap();
    assert_eq!(cached.texture, allocated);
    assert_eq!(cached.size, size);
    assert_ne!(cache.allocate_texture_id(SkinKind::Play), allocated);

    cache.clear();

    assert!(cache.get(&key).is_none());
    assert_eq!(cache.allocate_texture_id(SkinKind::Play), SkinTextureId(10_000));
}

#[test]
fn decode_uses_gpu_texture_cache_to_skip_source_decode() {
    let root = unique_test_dir("bmz-source-texture-cache-hit");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("skin.json");
    let source_path = root.join("source.png");
    std::fs::write(&source_path, b"not a png").unwrap();
    std::fs::write(
        &skin_path,
        r#"
            {
                "type": 0,
                "source": [
                    { "id": 1, "path": "source.png" }
                ],
                "image": [
                    { "id": "img", "src": 1, "x": 0, "y": 0, "w": 64, "h": 32 }
                ],
                "destination": [
                    { "id": "img", "dst": [{ "x": 0, "y": 0, "w": 64, "h": 32 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let key = skin_source_asset_cache_key(&source_path, false).unwrap();
    let texture = SkinTextureId(12_345);
    let size = SkinImageSize { width: 64.0, height: 32.0 };
    let texture_cache = Arc::new(Mutex::new(SkinGpuTextureCache::default()));
    texture_cache.lock().unwrap().insert(key.clone(), texture, size);

    let decoded = decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        None,
        None,
        Some(texture_cache),
        None,
        None,
    )
    .unwrap();

    assert_eq!(decoded.stats.source_texture_cache_hits, 1);
    assert_eq!(decoded.stats.source_texture_cache_hit_bytes, 64 * 32 * 4);
    assert_eq!(decoded.stats.source_cache_hits, 0);
    assert_eq!(decoded.stats.source_cache_misses, 0);
    assert_eq!(decoded.stats.decoded_source_bytes, 0);
    assert_eq!(decoded.sources.len(), 1);
    assert_eq!(decoded.sources[0].texture, texture);
    assert_eq!(decoded.sources[0].size, size);
    assert_eq!(decoded.sources[0].cache_key.as_ref(), Some(&key));
    assert!(decoded.sources[0].asset.is_none());
}

#[test]
fn skin_gpu_texture_cache_reuses_inserted_video_textures_separately() {
    let root = unique_test_dir("bmz-gpu-video-texture-cache");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("source.mp4");
    std::fs::write(&path, b"cached-video").unwrap();
    let image_key = skin_source_asset_cache_key(&path, false).unwrap();
    let video_key = skin_source_asset_cache_key(&path, true).unwrap();
    assert_ne!(image_key, video_key);

    let size = SkinImageSize { width: 320.0, height: 180.0 };
    let mut cache = SkinGpuTextureCache::default();
    let allocated = cache.allocate_texture_id(SkinKind::Play);
    cache.insert(video_key.clone(), allocated, size);

    assert!(cache.get(&image_key).is_none());
    let cached = cache.get(&video_key).unwrap();
    assert_eq!(cached.texture, allocated);
    assert_eq!(cached.size, size);
}
