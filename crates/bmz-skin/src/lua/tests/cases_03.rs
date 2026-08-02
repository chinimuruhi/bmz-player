use super::*;

#[test]
fn property_options_accept_integral_lua_numbers() {
    let property: JsonValue = serde_json::from_str(
        r#"
            {
                "name": "Key Beam Length",
                "def": "100%",
                "item": [
                    { "name": "100%", "op": 11400.0 },
                    { "name": "90%", "op": 11401.0 }
                ]
            }
            "#,
    )
    .unwrap();
    let header = serde_json::json!({ "property": [property] });
    let mut warnings = Vec::new();

    let options = skin_config_options_from_header(
        &header,
        &BTreeMap::from([("Key Beam Length".to_string(), "90%".to_string())]),
        &mut warnings,
    );

    assert_eq!(options.get("Key Beam Length"), Some(&11401));
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn property_options_reject_fractional_lua_numbers() {
    let items = vec![serde_json::json!({ "name": "invalid", "op": 11400.5 })];

    assert_eq!(option_value_to_op(&items, "invalid"), None);
}

#[test]
fn get_path_accepts_beatoraja_filename_selection() {
    let root = unique_skin_test_dir("filename-getpath");
    fs::create_dir_all(root.join("bg")).unwrap();
    fs::write(root.join("bg/one.mp4"), []).unwrap();
    let skin_files = BTreeMap::from([("bg/*.mp4".to_string(), "one.mp4".to_string())]);
    let path_context = test_skin_path_context(&root);

    let resolved = skin_config_get_path(&path_context, "bg/*.mp4", &skin_files).unwrap();

    assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("one.mp4"));
}

#[test]
fn get_path_randomizes_when_selection_is_random_sentinel() {
    let root = unique_skin_test_dir("random-getpath");
    fs::create_dir_all(root.join("bg")).unwrap();
    fs::write(root.join("bg/one.mp4"), []).unwrap();
    fs::write(root.join("bg/two.mp4"), []).unwrap();
    let skin_files = BTreeMap::from([("bg/*.mp4".to_string(), RANDOM_FILE_SELECTION.to_string())]);
    let path_context = test_skin_path_context(&root);

    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        let resolved = skin_config_get_path(&path_context, "bg/*.mp4", &skin_files).unwrap();
        let name =
            resolved.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string();
        assert!(name == "one.mp4" || name == "two.mp4", "unexpected match {name}");
        seen.insert(name);
    }
    assert_eq!(seen.len(), 2, "Random selection should pick randomly among matches");
}

#[test]
fn get_path_returns_sandboxed_path_before_file_exists() {
    let root = unique_skin_test_dir("missing-getpath");
    let entry = root.join("select.luaskin");
    fs::write(&entry, "return { type = 5 }").unwrap();
    let path_context = SkinPathContext::for_entry(&entry).unwrap();

    let resolved =
        skin_config_get_path(&path_context, "History/2026-08-03/history.txt", &BTreeMap::new())
            .unwrap();

    assert_eq!(resolved, root.join("History/2026-08-03/history.txt"));
    assert!(!resolved.exists());
    assert!(skin_config_get_path(&path_context, "../outside.txt", &BTreeMap::new()).is_err());
}

#[test]
fn repairs_strictly_recognized_malformed_destination_ops() {
    let mut value = serde_json::json!({
        "type": 7,
        "destination": [
            {
                "id": "rankBig_AAA",
                "op": {
                    "1": 300,
                    "2": 920,
                    "loop": 100,
                    "filter": 1,
                    "dst": [{"x": 77, "y": 800, "w": 400, "h": 510}]
                }
            },
            {
                "id": "AAA_BG",
                "op": [90, [90, 300]],
                "dst": [{"x": 0, "y": 0, "w": 1, "h": 1}]
            }
        ]
    });
    let mut warnings =
        vec!["mixed lua table converted to object at $.destination[1].op".to_string()];

    postprocess_lua_skin_json(value.as_object_mut().unwrap(), &mut warnings);

    assert_eq!(value["destination"][0]["op"], serde_json::json!([300, 920]));
    assert_eq!(value["destination"][0]["loop"], 100);
    assert_eq!(value["destination"][0]["filter"], 1);
    assert!(value["destination"][0]["dst"].is_array());
    assert_eq!(value["destination"][1]["op"], serde_json::json!([90, 300]));
    assert_eq!(warnings, ["repaired 2 malformed destination op tables"]);

    let document: bmz_skin_document::SkinDocument =
        serde_json::from_value(value.clone()).expect("repaired destinations should decode");
    let destinations = document
        .destination
        .iter()
        .filter_map(|entry| match entry {
            bmz_skin_document::DestinationListEntry::Single(destination) => Some(destination),
            bmz_skin_document::DestinationListEntry::Conditional { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(destinations[0].op, [300, 920]);
    assert_eq!(destinations[1].op, [90, 300]);

    let once = value.clone();
    let warning_count = warnings.len();
    postprocess_lua_skin_json(value.as_object_mut().unwrap(), &mut warnings);
    assert_eq!(value, once);
    assert_eq!(warnings.len(), warning_count);
}

#[test]
fn leaves_ambiguous_destination_ops_unmodified() {
    let mut value = serde_json::json!({
        "destination": [
            {"id": "sparse", "op": {"1": 90, "3": 300, "dst": []}},
            {"id": "unknown", "op": {"1": 90, "custom": 1, "dst": []}},
            {"id": "conflict", "loop": 200, "op": {"1": 90, "loop": 100, "dst": []}},
            {"id": "different-prefix", "op": [90, [300]], "dst": []},
            {"id": "deep", "op": [90, [90, [300]]], "dst": []}
        ]
    });
    let original = value.clone();
    let mut warnings = Vec::new();

    postprocess_lua_skin_json(value.as_object_mut().unwrap(), &mut warnings);

    assert_eq!(value, original);
    assert!(warnings.is_empty());
}
