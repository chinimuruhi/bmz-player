use super::*;

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap().flatten() {
        if entry.file_name() == ".git" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn hub_fixture(name: &str) -> (PathBuf, PathBuf) {
    let root = unique_test_dir(name).join("skins");
    let entry_dir = root.join("GenericTheme-master/play");
    let hub = root.join("Hub");
    fs::create_dir_all(&entry_dir).unwrap();
    fs::create_dir_all(hub.join("modules")).unwrap();
    fs::create_dir_all(hub.join("parts")).unwrap();
    fs::create_dir_all(hub.join("extension/sample")).unwrap();

    let entry = entry_dir.join("Hub_play7.luaskin");
    fs::write(
        &entry,
        r#"
            PATH_SKIN = package.path
            assert(string.find(PATH_SKIN, "GenericTheme%-master") ~= nil)
            assert(package.cpath == "")
            assert(package.loadlib == nil)
            PATH_HUB = "?.lua;skin/Hub/?.lua"
            package.path = PATH_HUB
            local const = require("const")
            assert(const.kind == "hub")
            local t = require("main")
            package.path = PATH_SKIN
            if skin_config then return t.main() else return t.header end
        "#,
    )
    .unwrap();
    fs::write(hub.join("const.lua"), "return { kind = 'hub' }").unwrap();
    fs::write(
        hub.join("mode.lua"),
        r#"
            HUB_MODE_LOADS = (HUB_MODE_LOADS or 0) + 1
            return { name = "generic", loads = HUB_MODE_LOADS }
        "#,
    )
    .unwrap();
    fs::write(hub.join("nilmod.lua"), "HUB_NIL_LOADS = (HUB_NIL_LOADS or 0) + 1").unwrap();
    fs::write(
        hub.join("modules/sample.lua"),
        r#"
            local module_name = ...
            local count = 0
            package.loaded[module_name] = {
                marker = "package-loaded",
                draw = function()
                    count = count + 1
                    return count % 2 == 0
                end
            }
        "#,
    )
    .unwrap();
    fs::write(
        hub.join("main.lua"),
        r#"
            local mode1 = require("mode")
            local mode2 = require("mode")
            assert(mode1 == mode2 and HUB_MODE_LOADS == 1)
            local nil1 = require("nilmod")
            local nil2 = require("nilmod")
            assert(nil1 == true and nil2 == true and HUB_NIL_LOADS == 1)

            local t = {}
            t.header = {
                type = 0,
                name = "Synthetic Hub",
                filepath = {{
                    name = "Hub extension",
                    path = "../../Hub/extension/*|1|",
                    def = "sample"
                }}
            }
            function t.main()
                package.path = PATH_SKIN
                local original = require("play")
                local original2 = loadfile("skin/GenericTheme-master/play/play.lua")()
                assert(original.kind == "original" and original2.kind == "original")

                local extension_dir = skin_config.get_path("../../Hub/extension/*|1|")
                local extension = dofile(extension_dir .. "/parts.lua")

                package.path = PATH_HUB
                local sample = require("modules.sample")
                assert(sample.marker == "package-loaded")
                return {
                    type = 0,
                    name = "Synthetic Hub",
                    filepath = t.header.filepath,
                    source = {{ id = "hub-test", path = "../../Hub/parts/sample.png" }},
                    destination = {{
                        id = "hub-test",
                        draw = sample.draw,
                        dst = {{ x = extension.x, y = 0, w = 1, h = 1 }}
                    }}
                }
            end
            return t
        "#,
    )
    .unwrap();
    fs::write(entry_dir.join("play.lua"), "return { kind = 'original' }").unwrap();
    fs::write(hub.join("extension/sample/parts.lua"), "return { x = 7 }").unwrap();
    fs::write(hub.join("parts/sample.png"), b"synthetic-png").unwrap();
    (root, entry)
}

#[test]
fn package_aware_hub_load_uses_dynamic_package_path_in_every_vm() {
    let (library_root, entry) = hub_fixture("bmz-skin-package-aware-hub");
    let context = SkinPathContext::new(&entry, [library_root.clone()]).unwrap();

    let header = load_lua_skin_header_value_with_path_context(&context).unwrap();
    assert_eq!(header.value.get("name").and_then(JsonValue::as_str), Some("Synthetic Hub"));

    let mut loaded = load_lua_skin_with_path_context(
        &context,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(loaded.document.name, "Synthetic Hub");
    assert_eq!(loaded.files.get("Hub extension").map(String::as_str), Some("sample"));
    assert!(loaded.dependencies.loaded_files.keys().any(|path| path.ends_with("Hub/const.lua")));
    assert!(
        loaded
            .dependencies
            .loaded_files
            .keys()
            .any(|path| path.ends_with("Hub/modules/sample.lua"))
    );
    assert!(
        loaded
            .dependencies
            .loaded_files
            .keys()
            .any(|path| path.ends_with("Hub/extension/sample/parts.lua"))
    );

    let source = loaded.document.source.iter().find(|source| source.id == "hub-test").unwrap();
    assert_eq!(
        context.resolve_file(&source.path).unwrap(),
        fs::canonicalize(library_root.join("Hub/parts/sample.png")).unwrap()
    );

    let runtime = loaded.lua_runtime.as_mut().expect("runtime draw callback should be retained");
    assert_eq!(runtime.callback_count(), 1);
    let state = TestLuaMainState::default();
    assert!(!runtime.evaluate_draw(0, &state));
    assert!(runtime.evaluate_draw(0, &state));
}

#[test]
fn package_path_context_allows_only_paths_inside_library_root() {
    let (library_root, entry) = hub_fixture("bmz-skin-package-path-boundary");
    let context = SkinPathContext::new(&entry, [library_root.clone()]).unwrap();

    fs::write(entry.parent().unwrap().join("const.lua"), "return 'entry-local'").unwrap();
    assert_eq!(
        context.resolve_file("skin/Hub/const.lua").unwrap(),
        fs::canonicalize(library_root.join("Hub/const.lua")).unwrap()
    );
    assert_eq!(
        context.resolve_file(r"skin\Hub\modules\sample.lua").unwrap(),
        fs::canonicalize(library_root.join("Hub/modules/sample.lua")).unwrap()
    );

    assert_eq!(
        context.resolve_file(r"..\..\Hub\parts\sample.png").unwrap(),
        fs::canonicalize(library_root.join("Hub/parts/sample.png")).unwrap()
    );
    let inside_absolute = fs::canonicalize(library_root.join("Hub/const.lua")).unwrap();
    assert_eq!(
        context.resolve_file(inside_absolute.to_string_lossy().as_ref()).unwrap(),
        inside_absolute
    );

    let outside = library_root.parent().unwrap().join("outside.lua");
    fs::write(&outside, "return 'outside'").unwrap();
    assert!(context.resolve_file("../../../outside.lua").is_err());
    assert!(context.resolve_file(outside.to_string_lossy().as_ref()).is_err());
    assert!(context.resolve_file("C:\\outside.lua").is_err());
    assert!(context.resolve_file("//server/share/outside.lua").is_err());
    assert!(context.resolve_file("bad\0path.lua").is_err());
}

#[test]
fn lua_to_json_conversion_uses_explicit_skin_library_roots() {
    let library_root = unique_test_dir("bmz-skin-package-convert").join("skins");
    let entry_dir = library_root.join("GenericTheme-master/play");
    fs::create_dir_all(&entry_dir).unwrap();
    fs::create_dir_all(library_root.join("Hub")).unwrap();
    let entry = entry_dir.join("Hub_play7.luaskin");
    fs::write(
        &entry,
        r#"
            package.path = "skin/Hub/?.lua"
            return require("convert_skin")
        "#,
    )
    .unwrap();
    fs::write(
        library_root.join("Hub/convert_skin.lua"),
        "return { type = 0, name = 'Converted Hub' }",
    )
    .unwrap();
    let context = SkinPathContext::new(&entry, [library_root]).unwrap();
    let output = entry_dir.join("converted.json");

    convert_lua_skin_to_json_file_with_path_context(
        &context,
        &output,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    let converted: JsonValue = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(converted.get("name").and_then(JsonValue::as_str), Some("Converted Hub"));
}

#[cfg(unix)]
#[test]
fn package_path_context_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let (library_root, entry) = hub_fixture("bmz-skin-package-symlink-boundary");
    let context = SkinPathContext::new(&entry, [library_root.clone()]).unwrap();
    let outside = library_root.parent().unwrap().join("outside-target.lua");
    fs::write(&outside, "return 'outside'").unwrap();
    symlink(&outside, library_root.join("Hub/escape.lua")).unwrap();

    assert!(context.resolve_file("skin/Hub/escape.lua").is_err());
}

#[test]
fn real_hub_wraps_generic_theme_when_local_copies_are_available() {
    let data_skins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins");
    let generic_source = data_skins.join("GenericTheme");
    let hub_package_source = data_skins.join("Hub/Hub");
    let wrapper_source = data_skins.join("Hub/Hub_play7.luaskin");
    if !generic_source.join("play/play.lua").is_file()
        || !hub_package_source.join("main.lua").is_file()
        || !wrapper_source.is_file()
    {
        return;
    }

    let library_root = unique_test_dir("bmz-skin-real-hub").join("skins");
    copy_tree(&generic_source, &library_root.join("GenericTheme-master"));
    copy_tree(&hub_package_source, &library_root.join("Hub"));
    let entry = library_root.join("GenericTheme-master/play/Hub_play7.luaskin");
    fs::copy(wrapper_source, &entry).unwrap();
    let context = SkinPathContext::new(&entry, [library_root]).unwrap();

    let header = load_lua_skin_header_value_with_path_context(&context)
        .expect("real Hub wrapper header should load GenericTheme and Hub modules");
    assert!(
        header
            .value
            .get("name")
            .and_then(JsonValue::as_str)
            .is_some_and(|name| name.contains("GenericTheme") && name.contains("Hub"))
    );

    let loaded = load_lua_skin_with_path_context(
        &context,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        &BTreeMap::new(),
    )
    .expect("real Hub wrapper document should load with default customization");
    assert!(loaded.document.name.contains("GenericTheme"));
    assert!(loaded.document.name.contains("Hub"));
    assert!(!loaded.document.source.is_empty());
    assert!(loaded.dependencies.loaded_files.keys().any(|path| path.ends_with("Hub/main.lua")));
    assert!(
        loaded
            .dependencies
            .loaded_files
            .keys()
            .any(|path| path.ends_with("GenericTheme-master/play/play.lua"))
    );
}
