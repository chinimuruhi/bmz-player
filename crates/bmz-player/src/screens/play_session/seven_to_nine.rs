use super::*;
use std::collections::HashMap;

/// Converts a 7K beat chart to PMS 9K using beatoraja's six placement
/// patterns and three scratch-distribution modes.
///
/// The normal 7K arrange is applied before this function. Source Key1..7 are
/// copied to the selected consecutive PMS keys; Scratch is placed on the
/// configured scratch key or its remaining companion key. Scratch long-note
/// pairs remain on one destination lane.
pub fn apply_seven_to_nine(
    chart: &mut PlayableChart,
    pattern: SevenToNinePattern,
    conversion_type: SevenToNineType,
    no_mashing_threshold_ms: u32,
) -> bool {
    if chart.metadata.key_mode != KeyMode::K7 {
        return false;
    }

    let (key_start, scratch_key, rest_key) = placement(pattern);
    let key_destinations: [Lane; 7] = std::array::from_fn(|index| {
        Lane::from_pms_key(key_start + index as u8).expect("7-to-9 key placement")
    });
    let scratch_lane = Lane::from_pms_key(scratch_key).expect("7-to-9 scratch placement");
    let rest_lane = Lane::from_pms_key(rest_key).expect("7-to-9 rest placement");

    let mut source_notes = Vec::new();
    for lane in Lane::ALL {
        source_notes.extend(std::mem::take(&mut chart.lane_notes[lane.index()]));
    }
    source_notes.sort_by_key(|note| (note.time, note.tick, note.id));

    let mut destination_notes: [Vec<NoteEvent>; LANE_COUNT] = std::array::from_fn(|_| Vec::new());
    let mut destination_by_note_id = HashMap::new();
    let mut last_note_time = [i64::MIN / 4; 9];
    let mut active_scratch_long_note = None;
    let threshold_us = i64::from(no_mashing_threshold_ms).saturating_mul(1_000);

    for mut note in source_notes {
        let destination = match note.lane {
            Lane::Key1 => key_destinations[0],
            Lane::Key2 => key_destinations[1],
            Lane::Key3 => key_destinations[2],
            Lane::Key4 => key_destinations[3],
            Lane::Key5 => key_destinations[4],
            Lane::Key6 => key_destinations[5],
            Lane::Key7 => key_destinations[6],
            Lane::Scratch => {
                if note.kind == NoteKind::LongEnd {
                    active_scratch_long_note.unwrap_or_else(|| {
                        choose_scratch_destination(
                            scratch_lane,
                            rest_lane,
                            conversion_type,
                            note.time.0,
                            threshold_us,
                            &last_note_time,
                        )
                    })
                } else {
                    let selected = choose_scratch_destination(
                        scratch_lane,
                        rest_lane,
                        conversion_type,
                        note.time.0,
                        threshold_us,
                        &last_note_time,
                    );
                    if note.kind == NoteKind::LongStart {
                        active_scratch_long_note = Some(selected);
                    }
                    selected
                }
            }
            _ => continue,
        };

        if note.kind != NoteKind::Invisible {
            last_note_time[pms_index(destination)] = note.time.0;
        }
        if note.lane == Lane::Scratch && note.kind == NoteKind::LongEnd {
            active_scratch_long_note = None;
        }
        note.lane = destination;
        destination_by_note_id.insert(note.id, destination);
        destination_notes[destination.index()].push(note);
    }

    for pair in &mut chart.long_notes {
        if let Some(&destination) = destination_by_note_id.get(&pair.start_note_id) {
            pair.lane = destination;
        }
    }
    chart.long_notes.retain(|pair| destination_by_note_id.contains_key(&pair.start_note_id));
    for notes in &mut destination_notes {
        notes.sort_by_key(|note| (note.time, note.tick, note.id));
    }
    chart.lane_notes = destination_notes;
    chart.metadata.key_mode = KeyMode::K9;
    chart.total_notes = chart
        .lane_notes
        .iter()
        .flatten()
        .filter(|note| matches!(note.kind, NoteKind::Tap | NoteKind::LongStart))
        .count() as u32;
    tracing::info!(
        pattern = pattern.value(),
        conversion_type = conversion_type.value(),
        total_notes = chart.total_notes,
        "converted 7K chart to 9K"
    );
    true
}

fn placement(pattern: SevenToNinePattern) -> (u8, u8, u8) {
    match pattern {
        SevenToNinePattern::Sc1Key2To8 => (2, 1, 9),
        SevenToNinePattern::Sc1Key3To9 => (3, 1, 2),
        SevenToNinePattern::Sc2Key3To9 => (3, 2, 1),
        SevenToNinePattern::Sc8Key1To7 => (1, 8, 9),
        SevenToNinePattern::Sc9Key1To7 => (1, 9, 8),
        SevenToNinePattern::Sc9Key2To8 => (2, 9, 1),
    }
}

fn choose_scratch_destination(
    scratch_lane: Lane,
    rest_lane: Lane,
    conversion_type: SevenToNineType,
    now_us: i64,
    threshold_us: i64,
    last_note_time: &[i64; 9],
) -> Lane {
    let scratch_elapsed = now_us.saturating_sub(last_note_time[pms_index(scratch_lane)]);
    let rest_elapsed = now_us.saturating_sub(last_note_time[pms_index(rest_lane)]);
    match conversion_type {
        SevenToNineType::Fixed => scratch_lane,
        SevenToNineType::NoMashing => {
            if scratch_elapsed > threshold_us || scratch_elapsed >= rest_elapsed {
                scratch_lane
            } else {
                rest_lane
            }
        }
        SevenToNineType::Alternation => {
            if scratch_elapsed >= rest_elapsed {
                scratch_lane
            } else {
                rest_lane
            }
        }
    }
}

fn pms_index(lane: Lane) -> usize {
    lane.index() - Lane::Key1.index()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bmz_chart::hash::compute_chart_identity;
    use bmz_chart::model::{ChartMetadata, LongNoteStyle, SoundAssetRef};
    use bmz_core::ids::NoteId;
    use bmz_core::time::ChartTick;

    fn chart() -> PlayableChart {
        PlayableChart {
            identity: compute_chart_identity(b"seven-to-nine"),
            metadata: ChartMetadata { key_mode: KeyMode::K7, ..Default::default() },
            lane_notes: std::array::from_fn(|_| Vec::new()),
            long_notes: Vec::new(),
            bgm_events: Vec::new(),
            bga_events: Vec::new(),
            timing_events: Vec::new(),
            scroll_events: Vec::new(),
            speed_events: Vec::new(),
            judge_rank_events: Vec::new(),
            bgm_volume_events: Vec::new(),
            key_volume_events: Vec::new(),
            text_events: Vec::new(),
            bga_opacity_events: Vec::new(),
            bga_argb_events: Vec::new(),
            swbga_definitions: Vec::new(),
            bga_keybound_events: Vec::new(),
            bga_asset_by_bmp_key: HashMap::new(),
            bar_lines: Vec::new(),
            sounds: Vec::<SoundAssetRef>::new(),
            bga_assets: Vec::new(),
            total_notes: 0,
            end_time: TimeUs(2_000_000),
        }
    }

    fn note(id: u32, lane: Lane, kind: NoteKind, time_us: i64) -> NoteEvent {
        NoteEvent {
            id: NoteId(id),
            lane,
            kind,
            tick: ChartTick(time_us as u64),
            time: TimeUs(time_us),
            sound: None,
            layered_sounds: Vec::new(),
            damage: None,
        }
    }

    #[test]
    fn maps_all_six_patterns_like_beatoraja() {
        let expected = [
            (Lane::Key2, Lane::Key8, Lane::Key1),
            (Lane::Key3, Lane::Key9, Lane::Key1),
            (Lane::Key3, Lane::Key9, Lane::Key2),
            (Lane::Key1, Lane::Key7, Lane::Key8),
            (Lane::Key1, Lane::Key7, Lane::Key9),
            (Lane::Key2, Lane::Key8, Lane::Key9),
        ];
        for (pattern, (first_key, last_key, scratch)) in
            SevenToNinePattern::VALUES.into_iter().zip(expected)
        {
            let mut chart = chart();
            chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, NoteKind::Tap, 1_000));
            chart.lane_notes[Lane::Key7.index()].push(note(2, Lane::Key7, NoteKind::Tap, 2_000));
            chart.lane_notes[Lane::Scratch.index()].push(note(
                3,
                Lane::Scratch,
                NoteKind::Tap,
                3_000,
            ));

            assert!(apply_seven_to_nine(&mut chart, pattern, SevenToNineType::Fixed, 125));
            assert!(chart.lane_notes[first_key.index()].iter().any(|note| note.id == NoteId(1)));
            assert!(chart.lane_notes[last_key.index()].iter().any(|note| note.id == NoteId(2)));
            assert!(chart.lane_notes[scratch.index()].iter().any(|note| note.id == NoteId(3)));
            assert_eq!(chart.metadata.key_mode, KeyMode::K9);
        }
    }

    #[test]
    fn alternation_preserves_scratch_long_note_lane() {
        let mut chart = chart();
        chart.lane_notes[Lane::Scratch.index()].extend([
            note(1, Lane::Scratch, NoteKind::LongStart, 1_000),
            note(2, Lane::Scratch, NoteKind::LongEnd, 500_000),
            note(3, Lane::Scratch, NoteKind::Tap, 600_000),
        ]);
        chart.long_notes.push(LongNotePair {
            lane: Lane::Scratch,
            style: LongNoteStyle::ChannelPair,
            mode: None,
            start_note_id: NoteId(1),
            end_note_id: NoteId(2),
            start_tick: ChartTick(1_000),
            end_tick: ChartTick(500_000),
            start_time: TimeUs(1_000),
            end_time: TimeUs(500_000),
            sound: None,
        });

        apply_seven_to_nine(
            &mut chart,
            SevenToNinePattern::Sc9Key1To7,
            SevenToNineType::Alternation,
            125,
        );

        let pair = &chart.long_notes[0];
        assert_eq!(pair.lane, Lane::Key9);
        assert!(chart.lane_notes[Lane::Key9.index()].iter().any(|note| note.id == NoteId(2)));
        assert!(chart.lane_notes[Lane::Key8.index()].iter().any(|note| note.id == NoteId(3)));
    }
}
