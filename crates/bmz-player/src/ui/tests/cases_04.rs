use super::*;

#[test]
fn direct_skin_reload_request_maps_battle_slots_and_offsets() {
    let mut request = SkinReloadRequest::default();
    request_skin_reload(&mut request, SkinSlot::Battle5, false);
    request_skin_reload(&mut request, SkinSlot::Battle7, true);

    assert!(request.play5);
    assert!(request.play7);
    assert!(!request.play10);
    assert!(!request.play14);
    assert!(request.offsets);
    assert!(!request.select);
    assert!(!request.result);
}

#[test]
fn skin_reload_diff_scopes_play_slot_without_select_reload() {
    let before = SkinConfig::default();
    let mut after = before.clone();
    after.play7_files.insert("Notes".to_string(), "blue.png".to_string());

    let request = skin_reload_request_from_diff(&before, &after);

    assert!(request.play7);
    assert!(!request.select);
    assert!(!request.play5);
    assert!(!request.result);
    assert!(request.any_reload());
}

#[test]
fn skin_reload_diff_separates_result_and_course_result_slots() {
    let before = SkinConfig::default();
    let mut after = before.clone();
    after.course_result = "data/skins/course/result.luaskin".to_string();
    after.course_result_options.insert("Layout".to_string(), "Course".to_string());

    let request = skin_reload_request_from_diff(&before, &after);

    assert!(request.course_result);
    assert!(!request.result);

    let mut after = before.clone();
    after.result_files.insert("Background".to_string(), "normal.png".to_string());

    let request = skin_reload_request_from_diff(&before, &after);

    assert!(request.result);
    assert!(!request.course_result);
}

#[test]
fn skin_reload_diff_marks_each_offset_slot_for_redecode() {
    type SkinReloadCase = (&'static str, fn(&mut SkinConfig), fn(SkinReloadRequest) -> bool);
    let cases: &[SkinReloadCase] = &[
        ("select", |skin| skin.select_offsets.push(Default::default()), |request| request.select),
        ("decide", |skin| skin.decide_offsets.push(Default::default()), |request| request.decide),
        ("play4", |skin| skin.play4_offsets.push(Default::default()), |request| request.play4),
        ("play5", |skin| skin.play5_offsets.push(Default::default()), |request| request.play5),
        ("play6", |skin| skin.play6_offsets.push(Default::default()), |request| request.play6),
        ("play7", |skin| skin.play7_offsets.push(Default::default()), |request| request.play7),
        ("play8", |skin| skin.play8_offsets.push(Default::default()), |request| request.play8),
        ("play9", |skin| skin.play9_offsets.push(Default::default()), |request| request.play9),
        ("play10", |skin| skin.play10_offsets.push(Default::default()), |request| request.play10),
        ("play14", |skin| skin.play14_offsets.push(Default::default()), |request| request.play14),
        ("battle5", |skin| skin.battle5_offsets.push(Default::default()), |request| request.play5),
        ("battle7", |skin| skin.battle7_offsets.push(Default::default()), |request| request.play7),
        ("result", |skin| skin.result_offsets.push(Default::default()), |request| request.result),
        (
            "course_result",
            |skin| skin.course_result_offsets.push(Default::default()),
            |request| request.course_result,
        ),
    ];

    for &(slot, change, slot_requested) in cases {
        let before = SkinConfig::default();
        let mut after = before.clone();
        change(&mut after);

        let request = skin_reload_request_from_diff(&before, &after);

        assert!(request.offsets, "{slot} offset did not mark runtime offset update");
        assert!(slot_requested(request), "{slot} offset did not mark scene re-decode");
        assert!(request.any_reload(), "{slot} offset did not request reload");
        assert!(request.any());
    }
}
