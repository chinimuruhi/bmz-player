use super::*;

#[test]
fn beatoraja_note_index_maps_4k_lanes_without_scratch() {
    assert_eq!(beatoraja_note_index(Lane::Key1, KeyMode::K4), 0);
    assert_eq!(beatoraja_note_index(Lane::Key2, KeyMode::K4), 1);
    assert_eq!(beatoraja_note_index(Lane::Key3, KeyMode::K4), 2);
    assert_eq!(beatoraja_note_index(Lane::Key4, KeyMode::K4), 3);
    assert_eq!(beatoraja_note_index(Lane::Scratch, KeyMode::K4), 3);
}

#[test]
fn beatoraja_note_index_maps_8k_lanes_without_scratch() {
    assert_eq!(beatoraja_note_index(Lane::Key1, KeyMode::K8), 0);
    assert_eq!(beatoraja_note_index(Lane::Key2, KeyMode::K8), 1);
    assert_eq!(beatoraja_note_index(Lane::Key3, KeyMode::K8), 2);
    assert_eq!(beatoraja_note_index(Lane::Key4, KeyMode::K8), 3);
    assert_eq!(beatoraja_note_index(Lane::Key5, KeyMode::K8), 4);
    assert_eq!(beatoraja_note_index(Lane::Key6, KeyMode::K8), 5);
    assert_eq!(beatoraja_note_index(Lane::Key7, KeyMode::K8), 6);
    assert_eq!(beatoraja_note_index(Lane::Key8, KeyMode::K8), 7);
    assert_eq!(beatoraja_note_index(Lane::Scratch, KeyMode::K8), 0);
}
