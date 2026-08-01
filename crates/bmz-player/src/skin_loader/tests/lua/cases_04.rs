use super::*;

#[test]
fn lua_cross_package_source_decodes_with_explicit_library_root() {
    let root = unique_test_dir("lua-cross-package-source").join("skins");
    let entry_dir = root.join("GenericTheme-master/play");
    let hub_parts = root.join("Hub/parts");
    fs::create_dir_all(&entry_dir).unwrap();
    fs::create_dir_all(&hub_parts).unwrap();
    let entry = entry_dir.join("Hub_play7.luaskin");
    fs::write(
        &entry,
        r#"
            return {
                type = 0,
                source = {{ id = "hub-test", path = "../../Hub/parts/sample.png" }},
                image = {{ id = "hub-image", src = "hub-test", x = 0, y = 0, w = 1, h = 1 }},
                destination = {{
                    id = "hub-image",
                    dst = {{ x = 0, y = 0, w = 1, h = 1 }}
                }}
            }
        "#,
    )
    .unwrap();
    let bundled_png =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/default/note.png");
    fs::copy(&bundled_png, hub_parts.join("sample.png")).unwrap();

    let options = BTreeMap::new();
    let files = BTreeMap::new();
    let runtime_state = LuaLoadRuntimeState::default();
    let decoded = decode_beatoraja_skin_request(BeatorajaSkinDecodeRequest {
        skin_path: &entry,
        kind: SkinKind::Play,
        options: &options,
        files: &files,
        runtime_state: &runtime_state,
        library_roots: std::slice::from_ref(&root),
        document_cache: None,
        source_cache: None,
        texture_cache: None,
        font_cache: None,
        installed_fonts: None,
    })
    .unwrap();

    let source = decoded.sources.iter().find(|source| source.source_id == "hub-test").unwrap();
    assert_eq!(source.path, fs::canonicalize(hub_parts.join("sample.png")).unwrap());
    assert!(source.asset.is_some());

    let context = SkinPathContext::new(&entry, [root]).unwrap();
    assert_eq!(
        resolve_skin_audio_path_with_context(
            context.entry_dir(),
            Some(&context),
            "../../Hub/parts/sample.png",
        )
        .unwrap(),
        source.path
    );
}
