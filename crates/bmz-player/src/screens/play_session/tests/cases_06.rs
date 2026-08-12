use super::*;

fn time_for_logical_frame(frame: i64) -> i64 {
    (i128::from(frame) * 1_000_000).div_euclid(120) as i64
        + i64::from((i128::from(frame) * 1_000_000).rem_euclid(120) != 0)
}

fn sequence_chart(key_mode: KeyMode, lane: Lane, frames: &[i64]) -> PlayableChart {
    let mut result = chart();
    result.metadata.key_mode = key_mode;
    for (index, &frame) in frames.iter().enumerate() {
        let time = time_for_logical_frame(frame);
        result.lane_notes[lane.index()].push(note((index + 1) as u32, lane, time));
    }
    result
}

fn lane_for_id(chart: &PlayableChart, id: u32) -> Lane {
    chart
        .lane_notes
        .iter()
        .flatten()
        .find(|note| note.id == NoteId(id))
        .map(|note| note.lane)
        .expect("note id must exist")
}

fn lanes_for_id_range(chart: &PlayableChart, ids: std::ops::RangeInclusive<u32>) -> Vec<Lane> {
    ids.map(|id| lane_for_id(chart, id)).collect()
}

#[test]
fn logical_frame_120_uses_absolute_integer_time_with_euclidean_division() {
    assert_eq!(logical_frame_120(TimeUs(0)), 0);
    assert_eq!(logical_frame_120(TimeUs(1)), 0);
    assert_eq!(logical_frame_120(TimeUs(8_333)), 0);
    assert_eq!(logical_frame_120(TimeUs(8_334)), 1);

    for (frames, boundary_us) in [(6, 50_000), (7, 58_334), (8, 66_667)] {
        assert_eq!(logical_frame_120(TimeUs(boundary_us - 1)), frames - 1);
        assert_eq!(logical_frame_120(TimeUs(boundary_us)), frames);
        assert_eq!(logical_frame_120(TimeUs(boundary_us + 1)), frames);
    }

    assert_eq!(logical_frame_120(TimeUs(-1)), -1);
    assert_eq!(logical_frame_120(TimeUs(-8_333)), -1);
    assert_eq!(logical_frame_120(TimeUs(-8_334)), -2);
    assert_eq!(logical_frame_120(TimeUs(-50_000)), -6);

    for time in [i64::MIN, i64::MAX] {
        let expected = (i128::from(time) * 120).div_euclid(1_000_000) as i64;
        assert_eq!(logical_frame_120(TimeUs(time)), expected);
    }
}

#[test]
fn lm_candidate_classification_matches_inclusive_6f_7f_8f_boundaries() {
    assert_eq!(
        classify_lm_candidate(LaneHistory { last_frame: Some(10), rapid_streak: 1 }, 16,),
        LmCandidateClass::TwoPlusWithin6F
    );
    assert_eq!(
        classify_lm_candidate(LaneHistory { last_frame: Some(10), rapid_streak: 1 }, 17,),
        LmCandidateClass::Safe
    );
    assert_eq!(
        classify_lm_candidate(LaneHistory { last_frame: Some(10), rapid_streak: 2 }, 17,),
        LmCandidateClass::ThreePlusWithin7F
    );
    assert_eq!(
        classify_lm_candidate(LaneHistory { last_frame: Some(10), rapid_streak: 2 }, 18,),
        LmCandidateClass::Safe
    );
    assert_eq!(
        classify_lm_candidate(LaneHistory { last_frame: Some(10), rapid_streak: 3 }, 18,),
        LmCandidateClass::FourPlusWithin8F
    );

    let (reset, gap) =
        next_lane_history(LaneHistory { last_frame: Some(10), rapid_streak: u16::MAX }, 19);
    assert_eq!(gap, 9);
    assert_eq!(reset.rapid_streak, 1);
    assert_eq!(classify_lm_candidate(reset, 28), LmCandidateClass::Safe);
    let (saturated, _) =
        next_lane_history(LaneHistory { last_frame: Some(20), rapid_streak: u16::MAX }, 28);
    assert_eq!(saturated.rapid_streak, u16::MAX);
}

#[test]
fn bpm255_sixteenths_quantize_to_sixteen_7f_and_one_8f_intervals() {
    let frames: Vec<i64> =
        (0_i64..=17).map(|index| logical_frame_120(TimeUs(index * 1_000_000 / 17))).collect();
    let deltas: Vec<i64> = frames.windows(2).map(|window| window[1] - window[0]).collect();

    assert_eq!(deltas.iter().filter(|&&delta| delta == 7).count(), 16);
    assert_eq!(deltas.iter().filter(|&&delta| delta == 8).count(), 1);
    assert_eq!(deltas.iter().sum::<i64>(), 120);
    assert!(deltas.iter().all(|&delta| matches!(delta, 7 | 8)));
}

#[test]
fn lm_correction_avoids_6f_doubles_7f_triples_and_8f_quadruples_when_safe() {
    for seed in [0, 1, 42, 0x00ff_ffff] {
        let mut six = sequence_chart(KeyMode::K7, Lane::Key1, &[120, 126, 132, 138, 144, 150]);
        apply_arrange(&mut six, ArrangeOption::SRandom, Some(seed), None);
        let six_lanes = lanes_for_id_range(&six, 1..=6);
        assert!(six_lanes.windows(2).all(|window| window[0] != window[1]), "seed={seed}");

        let mut seven = sequence_chart(KeyMode::K7, Lane::Key1, &[120, 127, 134, 141, 148, 155]);
        apply_arrange(&mut seven, ArrangeOption::SRandom, Some(seed), None);
        let seven_lanes = lanes_for_id_range(&seven, 1..=6);
        assert!(
            seven_lanes
                .windows(3)
                .all(|window| !(window[0] == window[1] && window[1] == window[2])),
            "seed={seed}"
        );

        let mut eight = sequence_chart(KeyMode::K7, Lane::Key1, &[120, 128, 136, 144, 152, 160]);
        apply_arrange(&mut eight, ArrangeOption::SRandom, Some(seed), None);
        let eight_lanes = lanes_for_id_range(&eight, 1..=6);
        assert!(
            eight_lanes.windows(4).all(|window| {
                !(window[0] == window[1] && window[1] == window[2] && window[2] == window[3])
            }),
            "seed={seed}"
        );
    }
}

#[test]
fn lm_candidate_shortage_relaxes_8f_then_7f_then_6f_and_keeps_a_bijection() {
    let lanes = [Lane::Key1, Lane::Key2, Lane::Key3, Lane::Key4];
    for note_count in 1..=3 {
        let mut group = NoteArrangeGroup::new(&lanes.map(Lane::index));
        group.active_ln.insert(Lane::Key4.index(), Lane::Key4.index());
        group.lane_history[Lane::Key1.index()] =
            LaneHistory { last_frame: Some(92), rapid_streak: 3 };
        group.lane_history[Lane::Key2.index()] =
            LaneHistory { last_frame: Some(93), rapid_streak: 2 };
        group.lane_history[Lane::Key3.index()] =
            LaneHistory { last_frame: Some(94), rapid_streak: 1 };
        let time = time_for_logical_frame(100);
        let notes: Vec<_> = lanes[..note_count]
            .iter()
            .enumerate()
            .map(|(index, &lane)| note((index + 1) as u32, lane, time))
            .collect();
        let mut rng = ArrangeRng::new(42, false);

        let map = group.lm_120hz_shuffle(&notes, TimeUs(time), &mut rng);
        let selected: HashSet<_> =
            lanes[..note_count].iter().filter_map(|lane| map.get(&lane.index()).copied()).collect();
        let expected: HashSet<_> = lanes[..note_count].iter().map(|lane| lane.index()).collect();
        assert_eq!(selected, expected, "note_count={note_count}");
        assert_eq!(map.len(), lanes.len());
        assert_eq!(map.values().copied().collect::<HashSet<_>>().len(), lanes.len());
        assert_eq!(map[&Lane::Key4.index()], Lane::Key4.index());
    }
}

#[test]
fn lm_violation_bucket_prefers_the_destination_with_the_larger_gap() {
    let lanes = [Lane::Key1.index(), Lane::Key2.index()];
    let mut group = NoteArrangeGroup::new(&lanes);
    group.lane_history[Lane::Key1.index()] = LaneHistory { last_frame: Some(94), rapid_streak: 1 };
    group.lane_history[Lane::Key2.index()] = LaneHistory { last_frame: Some(95), rapid_streak: 1 };
    let time = time_for_logical_frame(100);
    let notes = [note(1, Lane::Key1, time)];
    let mut rng = ArrangeRng::new(42, false);

    let map = group.lm_120hz_shuffle(&notes, TimeUs(time), &mut rng);

    assert_eq!(map[&Lane::Key1.index()], Lane::Key1.index());
}

fn lm_golden_chart() -> PlayableChart {
    let mut result = chart();
    result.metadata.key_mode = KeyMode::K7;
    for (id, lane, frame) in [
        (1, Lane::Key1, 120),
        (2, Lane::Key1, 126),
        (3, Lane::Key1, 133),
        (4, Lane::Key3, 133),
        (5, Lane::Key2, 141),
        (6, Lane::Scratch, 144),
        (7, Lane::Key4, 147),
        (8, Lane::Key7, 153),
        (9, Lane::Key4, 160),
    ] {
        let time = time_for_logical_frame(frame);
        result.lane_notes[lane.index()].push(note(id, lane, time));
    }
    result
}

fn replay_compatibility_chart() -> PlayableChart {
    let mut result = chart();
    result.metadata.key_mode = KeyMode::K7;
    for (id, lane, time) in [
        (1, Lane::Key1, 1_000_000),
        (2, Lane::Key1, 1_020_000),
        (3, Lane::Key1, 1_060_000),
        (4, Lane::Key3, 1_060_000),
        (5, Lane::Key2, 1_080_000),
        (6, Lane::Scratch, 1_100_000),
        (7, Lane::Key4, 1_150_000),
        (8, Lane::Key7, 1_170_000),
    ] {
        result.lane_notes[lane.index()].push(note(id, lane, time));
    }
    result
}

#[test]
fn replay_v4_fixture_rebuilds_the_pre_lm_s_random_layout() {
    let replay: crate::storage::replay::ReplayFile = toml::from_str(
        r#"
version = 4
chart_sha256 = "0101010101010101010101010101010101010101010101010101010101010101"
played_at = 1700000060
arrange = "SRandom"
arrange_seed = 1193046
seed_scheme = "beatoraja_24bit_v1"
events = []
"#,
    )
    .unwrap();
    let scheme = replay.effective_s_random_scheme().unwrap();
    assert_eq!(scheme, SRandomScheme::Legacy40MsV1);
    let mut arranged = replay_compatibility_chart();

    apply_arrange_internal(
        &mut arranged,
        replay.arrange_option(),
        replay.arrange_seed,
        replay.lane_shuffle_pattern.as_deref(),
        replay.uses_legacy_seed_scheme(),
        scheme,
    );

    assert_eq!(
        lanes_for_id_range(&arranged, 1..=8),
        vec![
            Lane::Key3,
            Lane::Key4,
            Lane::Key6,
            Lane::Key7,
            Lane::Key1,
            Lane::Scratch,
            Lane::Key3,
            Lane::Key6,
        ]
    );
}

#[test]
fn rng_and_s_random_scheme_combinations_are_deterministic_and_independent() {
    for legacy_rng in [false, true] {
        for scheme in [SRandomScheme::Legacy40MsV1, SRandomScheme::Lm120HzV1] {
            let mut first = replay_compatibility_chart();
            let mut second = replay_compatibility_chart();
            let first_applied = apply_arrange_internal(
                &mut first,
                ArrangeOption::SRandom,
                Some(0x12_3456),
                None,
                legacy_rng,
                scheme,
            );
            let second_applied = apply_arrange_internal(
                &mut second,
                ArrangeOption::SRandom,
                Some(0x12_3456),
                None,
                legacy_rng,
                scheme,
            );

            assert_eq!(lanes_for_notes(&first), lanes_for_notes(&second));
            assert_eq!(first_applied.legacy_seed, legacy_rng);
            assert_eq!(second_applied.s_random_scheme, scheme);
        }
    }
}

#[test]
fn lm_120hz_v1_has_fixed_golden_rng_order_and_is_deterministic() {
    // LM approximation v1 replay compatibility includes this exact RNG call order.
    let expected = vec![
        (NoteId(1), Lane::Key3),
        (NoteId(2), Lane::Key5),
        (NoteId(3), Lane::Key1),
        (NoteId(4), Lane::Key7),
        (NoteId(5), Lane::Key2),
        (NoteId(6), Lane::Scratch),
        (NoteId(7), Lane::Key4),
        (NoteId(8), Lane::Key1),
        (NoteId(9), Lane::Key7),
    ];

    for _ in 0..8 {
        let mut actual = lm_golden_chart();
        let applied = apply_arrange_internal(
            &mut actual,
            ArrangeOption::SRandom,
            Some(0x12_3456),
            None,
            false,
            SRandomScheme::Lm120HzV1,
        );
        assert_eq!(applied.s_random_scheme, SRandomScheme::Lm120HzV1);
        assert_eq!(lanes_for_notes(&actual), expected);
    }
}

fn arranged_dp_chart(seed_1p: i64, seed_2p: i64) -> PlayableChart {
    let mut result = chart();
    result.metadata.key_mode = KeyMode::K14;
    for index in 0..10_u32 {
        let time = time_for_logical_frame(120 + i64::from(index) * 6);
        result.lane_notes[Lane::Key1.index()].push(note(index + 1, Lane::Key1, time));
        result.lane_notes[Lane::Key8.index()].push(note(index + 101, Lane::Key8, time));
    }
    let applied = apply_arrange_pair(
        &mut result,
        ArrangeOption::SRandom,
        ArrangeOption::SRandom,
        Some(seed_1p),
        Some(seed_2p),
        false,
        SRandomScheme::Lm120HzV1,
        Some(SRandomScheme::Lm120HzV1),
        None,
    );
    assert_eq!(applied.seed, Some(seed_1p));
    assert_eq!(applied.seed_2p, Some(seed_2p));
    assert_eq!(applied.s_random_scheme, SRandomScheme::Lm120HzV1);
    assert_eq!(applied.s_random_scheme_2p, Some(SRandomScheme::Lm120HzV1));
    result
}

#[test]
fn lm_dp_uses_independent_side_seeds_and_never_crosses_lane_groups() {
    let original = arranged_dp_chart(11, 22);
    let changed_2p = arranged_dp_chart(11, 23);
    let changed_1p = arranged_dp_chart(12, 22);
    let original_1p = lanes_for_id_range(&original, 1..=10);
    let original_2p = lanes_for_id_range(&original, 101..=110);

    assert_eq!(original_1p, lanes_for_id_range(&changed_2p, 1..=10));
    assert_ne!(original_2p, lanes_for_id_range(&changed_2p, 101..=110));
    assert_eq!(original_2p, lanes_for_id_range(&changed_1p, 101..=110));
    assert_ne!(original_1p, lanes_for_id_range(&changed_1p, 1..=10));

    assert!(
        original_1p
            .iter()
            .all(|lane| (Lane::Key1.index()..=Lane::Key7.index()).contains(&lane.index()))
    );
    assert!(
        original_2p
            .iter()
            .all(|lane| (Lane::Key8.index()..=Lane::Key14.index()).contains(&lane.index()))
    );
    assert!(original_1p.windows(2).all(|window| window[0] != window[1]));
    assert!(original_2p.windows(2).all(|window| window[0] != window[1]));
}

#[test]
fn lm_long_note_reserves_its_destination_until_the_matching_end() {
    use bmz_chart::model::{LongNoteMode, LongNotePair, LongNoteStyle};
    use bmz_core::time::ChartTick;

    let mut result = chart();
    result.metadata.key_mode = KeyMode::K7;
    let start_time = time_for_logical_frame(120);
    let tap_time = time_for_logical_frame(126);
    let end_time = time_for_logical_frame(140);
    result.lane_notes[Lane::Key1.index()]
        .push(NoteEvent { kind: NoteKind::LongStart, ..note(1, Lane::Key1, start_time) });
    result.lane_notes[Lane::Key2.index()].push(note(2, Lane::Key2, tap_time));
    result.lane_notes[Lane::Key1.index()]
        .push(NoteEvent { kind: NoteKind::LongEnd, ..note(3, Lane::Key1, end_time) });
    result.long_notes.push(LongNotePair {
        lane: Lane::Key1,
        style: LongNoteStyle::ChannelPair,
        mode: Some(LongNoteMode::Cn),
        start_note_id: NoteId(1),
        end_note_id: NoteId(3),
        start_tick: ChartTick((start_time / 1_000) as u64),
        end_tick: ChartTick((end_time / 1_000) as u64),
        start_time: TimeUs(start_time),
        end_time: TimeUs(end_time),
        sound: None,
    });

    apply_arrange(&mut result, ArrangeOption::SRandom, Some(5), None);

    let start_lane = lane_for_id(&result, 1);
    assert_eq!(lane_for_id(&result, 3), start_lane);
    assert_ne!(lane_for_id(&result, 2), start_lane);
    assert_eq!(result.long_notes[0].lane, start_lane);
}

#[test]
fn lm_mine_only_timeline_does_not_change_following_arrange_or_history() {
    let make_chart = |with_mine: bool| {
        let mut result = sequence_chart(KeyMode::K7, Lane::Key1, &[120, 126, 132, 138]);
        if with_mine {
            let time = time_for_logical_frame(123);
            result.lane_notes[Lane::Key3.index()].push(NoteEvent {
                kind: NoteKind::Mine,
                damage: Some(25.0),
                ..note(99, Lane::Key3, time)
            });
        }
        result
    };
    let mut without_mine = make_chart(false);
    let mut with_mine = make_chart(true);

    apply_arrange(&mut without_mine, ArrangeOption::SRandom, Some(77), None);
    apply_arrange(&mut with_mine, ArrangeOption::SRandom, Some(77), None);

    assert_eq!(lanes_for_id_range(&without_mine, 1..=4), lanes_for_id_range(&with_mine, 1..=4));
    assert!(KeyMode::K7.active_lanes().contains(&lane_for_id(&with_mine, 99)));
}

#[test]
fn s_random_ex_applies_lm_correction_to_scratch_while_normal_s_random_excludes_it() {
    let frames = [120, 126, 132, 138, 144, 150, 156, 162];
    let mut normal = sequence_chart(KeyMode::K7, Lane::Scratch, &frames);
    let mut extended = normal.clone();

    apply_arrange(&mut normal, ArrangeOption::SRandom, Some(9), None);
    apply_arrange(&mut extended, ArrangeOption::SRandomEx, Some(9), None);

    let normal_lanes = lanes_for_id_range(&normal, 1..=8);
    let extended_lanes = lanes_for_id_range(&extended, 1..=8);
    assert!(normal_lanes.iter().all(|&lane| lane == Lane::Scratch));
    assert!(extended_lanes.iter().any(|&lane| lane != Lane::Scratch));
    assert!(extended_lanes.windows(2).all(|window| window[0] != window[1]));
    assert!(extended_lanes.iter().all(|lane| KeyMode::K7.active_lanes().contains(lane)));
}

#[test]
fn lm_s_random_keeps_a_complete_bijection_in_every_supported_key_mode() {
    for key_mode in [
        KeyMode::K4,
        KeyMode::K5,
        KeyMode::K6,
        KeyMode::K7,
        KeyMode::K8,
        KeyMode::K9,
        KeyMode::K10,
        KeyMode::K14,
    ] {
        let mut result = chart();
        result.metadata.key_mode = key_mode;
        let source_lanes: Vec<_> = key_mode
            .active_lanes()
            .iter()
            .copied()
            .filter(|lane| !matches!(lane, Lane::Scratch | Lane::Scratch2))
            .collect();
        for (index, &lane) in source_lanes.iter().enumerate() {
            result.lane_notes[lane.index()].push(note(
                (index + 1) as u32,
                lane,
                time_for_logical_frame(120),
            ));
        }

        if matches!(key_mode, KeyMode::K10 | KeyMode::K14) {
            apply_arrange_pair(
                &mut result,
                ArrangeOption::SRandom,
                ArrangeOption::SRandom,
                Some(11),
                Some(22),
                false,
                SRandomScheme::Lm120HzV1,
                None,
                None,
            );
        } else {
            apply_arrange(&mut result, ArrangeOption::SRandom, Some(11), None);
        }

        let destinations: HashSet<_> =
            result.lane_notes.iter().flatten().map(|event| event.lane).collect();
        assert_eq!(destinations, source_lanes.into_iter().collect(), "{key_mode:?}");
    }
}

#[test]
fn lm_same_source_objects_on_one_timeline_update_history_once() {
    let lanes = [Lane::Key1.index(), Lane::Key2.index()];
    let mut group = NoteArrangeGroup::new(&lanes);
    let time = time_for_logical_frame(120);
    let notes = [note(1, Lane::Key1, time), note(2, Lane::Key1, time)];
    let mut rng = ArrangeRng::new(3, false);

    let map = group.lm_120hz_shuffle(&notes, TimeUs(time), &mut rng);
    let destination = map[&Lane::Key1.index()];

    assert_eq!(group.lane_history[destination].rapid_streak, 1);
    assert_eq!(group.lane_history[destination].last_frame, Some(120));
}
