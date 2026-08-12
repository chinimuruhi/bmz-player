use super::*;

pub fn generate_arrange_seed() -> i64 {
    i64::from(RandomOptionSeeds::fresh(false).p1.value())
}

pub(super) fn normal_applied_arrange(seed: i64, legacy_seed: bool) -> AppliedArrange {
    AppliedArrange {
        arrange: ArrangeOption::Normal,
        arrange_2p: ArrangeOption::Normal,
        double_option: DoubleOption::Off,
        seed: Some(seed),
        seed_2p: None,
        legacy_seed,
        bms_random_choices: Vec::new(),
        pattern: None,
        seven_to_six: false,
    }
}

pub fn apply_arrange(
    chart: &mut PlayableChart,
    arrange: ArrangeOption,
    seed: Option<i64>,
    pattern: Option<&[u8]>,
) -> AppliedArrange {
    apply_arrange_internal(chart, arrange, seed, pattern, false)
}

pub(super) fn apply_arrange_internal(
    chart: &mut PlayableChart,
    arrange: ArrangeOption,
    seed: Option<i64>,
    pattern: Option<&[u8]>,
    legacy_seed: bool,
) -> AppliedArrange {
    let key_mode = chart.metadata.key_mode;
    let used_seed = seed.unwrap_or_else(generate_arrange_seed);
    if arrange_requires_scratch(arrange) && !key_mode_has_scratch(key_mode) {
        return normal_applied_arrange(used_seed, legacy_seed);
    }

    if let Some(perm) = pattern {
        let perm_usize: Vec<usize> = perm.iter().map(|&i| i as usize).collect();
        apply_lane_permutation(chart, &perm_usize);
        return AppliedArrange {
            arrange,
            arrange_2p: ArrangeOption::Normal,
            double_option: DoubleOption::Off,
            seed: Some(used_seed),
            seed_2p: None,
            legacy_seed,
            bms_random_choices: Vec::new(),
            pattern: Some(perm.to_vec()),
            seven_to_six: false,
        };
    }

    match arrange {
        ArrangeOption::Normal => normal_applied_arrange(used_seed, legacy_seed),
        ArrangeOption::Mirror => {
            let perm = mirror_permutation(key_mode);
            apply_lane_permutation(chart, &perm);
            AppliedArrange {
                arrange: ArrangeOption::Mirror,
                arrange_2p: ArrangeOption::Normal,
                double_option: DoubleOption::Off,
                seed: Some(used_seed),
                seed_2p: None,
                legacy_seed,
                bms_random_choices: Vec::new(),
                pattern: Some(perm.iter().map(|&i| i as u8).collect()),
                seven_to_six: false,
            }
        }
        ArrangeOption::Random => {
            let perm = random_lane_permutation(used_seed, key_mode, false, legacy_seed);
            apply_lane_permutation(chart, &perm);
            AppliedArrange {
                arrange: ArrangeOption::Random,
                arrange_2p: ArrangeOption::Normal,
                double_option: DoubleOption::Off,
                seed: Some(used_seed),
                seed_2p: None,
                legacy_seed,
                bms_random_choices: Vec::new(),
                pattern: Some(perm.iter().map(|&i| i as u8).collect()),
                seven_to_six: false,
            }
        }
        ArrangeOption::RRandom => {
            let perm = rotate_lane_permutation(used_seed, key_mode, false, legacy_seed);
            apply_lane_permutation(chart, &perm);
            AppliedArrange {
                arrange: ArrangeOption::RRandom,
                arrange_2p: ArrangeOption::Normal,
                double_option: DoubleOption::Off,
                seed: Some(used_seed),
                seed_2p: None,
                legacy_seed,
                bms_random_choices: Vec::new(),
                pattern: Some(perm.iter().map(|&i| i as u8).collect()),
                seven_to_six: false,
            }
        }
        ArrangeOption::RandomEx => {
            let perm = random_lane_permutation(used_seed, key_mode, true, legacy_seed);
            apply_lane_permutation(chart, &perm);
            AppliedArrange {
                arrange: ArrangeOption::RandomEx,
                arrange_2p: ArrangeOption::Normal,
                double_option: DoubleOption::Off,
                seed: Some(used_seed),
                seed_2p: None,
                legacy_seed,
                bms_random_choices: Vec::new(),
                pattern: Some(perm.iter().map(|&i| i as u8).collect()),
                seven_to_six: false,
            }
        }
        ArrangeOption::FRandom | ArrangeOption::MFRandom => {
            let perm = f_random_lane_permutation(used_seed, key_mode, arrange, legacy_seed);
            apply_lane_permutation(chart, &perm);
            AppliedArrange {
                arrange,
                arrange_2p: ArrangeOption::Normal,
                double_option: DoubleOption::Off,
                seed: Some(used_seed),
                seed_2p: None,
                legacy_seed,
                bms_random_choices: Vec::new(),
                pattern: Some(perm.iter().map(|&i| i as u8).collect()),
                seven_to_six: false,
            }
        }
        ArrangeOption::SRandom
        | ArrangeOption::Spiral
        | ArrangeOption::HRandom
        | ArrangeOption::AllScratch
        | ArrangeOption::SRandomEx => {
            apply_note_arrange(chart, arrange, used_seed, legacy_seed);
            AppliedArrange {
                arrange,
                arrange_2p: ArrangeOption::Normal,
                double_option: DoubleOption::Off,
                seed: Some(used_seed),
                seed_2p: None,
                legacy_seed,
                bms_random_choices: Vec::new(),
                pattern: None,
                seven_to_six: false,
            }
        }
    }
}

pub(super) fn arrange_requires_scratch(arrange: ArrangeOption) -> bool {
    matches!(
        arrange,
        ArrangeOption::AllScratch | ArrangeOption::RandomEx | ArrangeOption::SRandomEx
    )
}

pub(super) fn key_mode_has_scratch(key_mode: KeyMode) -> bool {
    key_mode.active_lanes().iter().any(|&lane| matches!(lane, Lane::Scratch | Lane::Scratch2))
}

pub fn apply_arrange_pair(
    chart: &mut PlayableChart,
    arrange_1p: ArrangeOption,
    arrange_2p: ArrangeOption,
    seed: Option<i64>,
    seed_2p: Option<i64>,
    legacy_seed: bool,
    pattern: Option<&[u8]>,
) -> AppliedArrange {
    let used_seed = seed.unwrap_or_else(generate_arrange_seed);
    let key_mode = chart.metadata.key_mode;
    if !matches!(key_mode, KeyMode::K10 | KeyMode::K14) {
        return apply_arrange_internal(chart, arrange_1p, Some(used_seed), pattern, legacy_seed);
    }

    let used_seed_2p = if legacy_seed {
        used_seed.wrapping_add(0x9e37_79b9)
    } else {
        seed_2p.unwrap_or_else(generate_arrange_seed)
    };
    if let Some(perm) = pattern {
        let perm_usize: Vec<usize> = perm.iter().map(|&i| i as usize).collect();
        apply_lane_permutation(chart, &perm_usize);
        return AppliedArrange {
            arrange: arrange_1p,
            arrange_2p,
            double_option: DoubleOption::Off,
            seed: Some(used_seed),
            seed_2p: Some(used_seed_2p),
            legacy_seed,
            bms_random_choices: Vec::new(),
            pattern: Some(perm.to_vec()),
            seven_to_six: false,
        };
    }

    let mut combined_perm: Vec<usize> = (0..LANE_COUNT).collect();
    let mut has_perm = false;

    if let Some(perm) =
        apply_arrange_side(chart, arrange_1p, Some(used_seed), ArrangeSide::P1, legacy_seed)
    {
        merge_lane_permutation(&mut combined_perm, &perm);
        has_perm = true;
    }
    if let Some(perm) =
        apply_arrange_side(chart, arrange_2p, Some(used_seed_2p), ArrangeSide::P2, legacy_seed)
    {
        merge_lane_permutation(&mut combined_perm, &perm);
        has_perm = true;
    }

    AppliedArrange {
        arrange: arrange_1p,
        arrange_2p,
        double_option: DoubleOption::Off,
        seed: Some(used_seed),
        seed_2p: Some(used_seed_2p),
        legacy_seed,
        bms_random_choices: Vec::new(),
        pattern: has_perm.then(|| combined_perm.iter().map(|&i| i as u8).collect()),
        seven_to_six: false,
    }
}

pub(super) fn apply_double_option(chart: &mut PlayableChart, double_option: DoubleOption) {
    match double_option {
        DoubleOption::Off => return,
        DoubleOption::Flip => {
            if !matches!(chart.metadata.key_mode, KeyMode::K10 | KeyMode::K14) {
                return;
            }
        }
        DoubleOption::Battle | DoubleOption::BattleAutoScratch => {
            apply_battle_double_option(chart);
            return;
        }
    }

    let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
    for (left, right) in [
        (Lane::Scratch, Lane::Scratch2),
        (Lane::Key1, Lane::Key8),
        (Lane::Key2, Lane::Key9),
        (Lane::Key3, Lane::Key10),
        (Lane::Key4, Lane::Key11),
        (Lane::Key5, Lane::Key12),
        (Lane::Key6, Lane::Key13),
        (Lane::Key7, Lane::Key14),
    ] {
        let left = left.index();
        let right = right.index();
        perm[left] = right;
        perm[right] = left;
    }
    apply_lane_permutation(chart, &perm);
}

pub(super) fn apply_battle_double_option(chart: &mut PlayableChart) {
    let (next_mode, pairs): (KeyMode, &[(Lane, Lane)]) = match chart.metadata.key_mode {
        KeyMode::K5 => (
            KeyMode::K10,
            &[
                (Lane::Scratch, Lane::Scratch2),
                (Lane::Key1, Lane::Key8),
                (Lane::Key2, Lane::Key9),
                (Lane::Key3, Lane::Key10),
                (Lane::Key4, Lane::Key11),
                (Lane::Key5, Lane::Key12),
            ],
        ),
        KeyMode::K7 => (
            KeyMode::K14,
            &[
                (Lane::Scratch, Lane::Scratch2),
                (Lane::Key1, Lane::Key8),
                (Lane::Key2, Lane::Key9),
                (Lane::Key3, Lane::Key10),
                (Lane::Key4, Lane::Key11),
                (Lane::Key5, Lane::Key12),
                (Lane::Key6, Lane::Key13),
                (Lane::Key7, Lane::Key14),
            ],
        ),
        _ => return,
    };

    let mut next_note_id = next_note_id(chart);
    let mut cloned_ids = std::collections::HashMap::new();
    for &(source, dest) in pairs {
        let source_index = source.index();
        let dest_index = dest.index();
        let clones: Vec<NoteEvent> = chart.lane_notes[source_index]
            .iter()
            .cloned()
            .map(|mut note| {
                let new_id = next_note_id;
                next_note_id.0 = next_note_id.0.saturating_add(1);
                cloned_ids.insert(note.id, new_id);
                note.id = new_id;
                note.lane = dest;
                note
            })
            .collect();
        chart.lane_notes[dest_index].extend(clones);
    }

    let source_to_dest: std::collections::HashMap<_, _> = pairs.iter().copied().collect();
    let mut cloned_long_notes = Vec::new();
    for pair in &chart.long_notes {
        let Some(&dest) = source_to_dest.get(&pair.lane) else {
            continue;
        };
        let (Some(&start_note_id), Some(&end_note_id)) =
            (cloned_ids.get(&pair.start_note_id), cloned_ids.get(&pair.end_note_id))
        else {
            continue;
        };
        let mut cloned = pair.clone();
        cloned.lane = dest;
        cloned.start_note_id = start_note_id;
        cloned.end_note_id = end_note_id;
        cloned_long_notes.push(cloned);
    }
    chart.long_notes.extend(cloned_long_notes);
    chart.total_notes = chart.total_notes.saturating_mul(2);
    chart.metadata.key_mode = next_mode;
}

pub(super) fn second_player_lane_mask() -> [bool; LANE_COUNT] {
    let mut mask = [false; LANE_COUNT];
    for lane in [
        Lane::Key8,
        Lane::Key9,
        Lane::Key10,
        Lane::Key11,
        Lane::Key12,
        Lane::Key13,
        Lane::Key14,
        Lane::Scratch2,
    ] {
        mask[lane.index()] = true;
    }
    mask
}

pub(super) fn next_note_id(chart: &PlayableChart) -> NoteId {
    let lane_max = chart.lane_notes.iter().flatten().map(|note| note.id.0).max().unwrap_or(0);
    let long_max = chart
        .long_notes
        .iter()
        .flat_map(|pair| [pair.start_note_id.0, pair.end_note_id.0])
        .max()
        .unwrap_or(0);
    NoteId(lane_max.max(long_max).saturating_add(1))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArrangeSide {
    P1,
    P2,
}
