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

pub(super) fn apply_arrange_side(
    chart: &mut PlayableChart,
    arrange: ArrangeOption,
    seed: Option<i64>,
    side: ArrangeSide,
    legacy_seed: bool,
) -> Option<Vec<usize>> {
    if arrange == ArrangeOption::Normal {
        return None;
    }

    let include_scratch = matches!(
        arrange,
        ArrangeOption::AllScratch | ArrangeOption::RandomEx | ArrangeOption::SRandomEx
    );
    let groups = arrange_lane_groups_for_side(chart.metadata.key_mode, include_scratch, side);
    if groups.is_empty() {
        return None;
    }

    match arrange {
        ArrangeOption::Normal => None,
        ArrangeOption::Mirror => {
            let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
            for group in groups {
                reverse_lane_group(&mut perm, &group);
            }
            apply_lane_permutation(chart, &perm);
            Some(perm)
        }
        ArrangeOption::Random => {
            let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
            let mut rng = ArrangeRng::new(seed.unwrap_or_else(generate_arrange_seed), legacy_seed);
            for group in groups {
                shuffle_lane_group(&mut rng, &group, &mut perm, legacy_seed);
            }
            apply_lane_permutation(chart, &perm);
            Some(perm)
        }
        ArrangeOption::RRandom => {
            let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
            let mut rng = ArrangeRng::new(seed.unwrap_or_else(generate_arrange_seed), legacy_seed);
            for group in groups {
                rotate_lane_group(&mut rng, &group, &mut perm);
            }
            apply_lane_permutation(chart, &perm);
            Some(perm)
        }
        ArrangeOption::RandomEx => {
            let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
            let mut rng = ArrangeRng::new(seed.unwrap_or_else(generate_arrange_seed), legacy_seed);
            for group in groups {
                shuffle_lane_group(&mut rng, &group, &mut perm, legacy_seed);
            }
            apply_lane_permutation(chart, &perm);
            Some(perm)
        }
        ArrangeOption::FRandom | ArrangeOption::MFRandom => {
            let perm = f_random_lane_permutation_for_side(
                seed.unwrap_or_else(generate_arrange_seed),
                chart.metadata.key_mode,
                arrange,
                side,
                legacy_seed,
            );
            apply_lane_permutation(chart, &perm);
            Some(perm)
        }
        ArrangeOption::SRandom
        | ArrangeOption::Spiral
        | ArrangeOption::HRandom
        | ArrangeOption::AllScratch
        | ArrangeOption::SRandomEx => {
            apply_note_arrange_for_groups(
                chart,
                arrange,
                seed.unwrap_or_else(generate_arrange_seed),
                &groups,
                legacy_seed,
            );
            None
        }
    }
}

pub(super) fn merge_lane_permutation(target: &mut [usize], source: &[usize]) {
    for (index, &source_lane) in source.iter().enumerate() {
        if source_lane != index {
            target[index] = source_lane;
        }
    }
}

pub(super) fn mirror_permutation(key_mode: KeyMode) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
    for group in arrange_lane_groups(key_mode, false) {
        reverse_lane_group(&mut perm, &group);
    }
    perm
}

pub(super) fn random_lane_permutation(
    seed: i64,
    key_mode: KeyMode,
    include_scratch: bool,
    legacy_seed: bool,
) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
    let mut rng = ArrangeRng::new(seed, legacy_seed);
    for group in arrange_lane_groups(key_mode, include_scratch) {
        shuffle_lane_group(&mut rng, &group, &mut perm, legacy_seed);
    }
    perm
}

pub(super) fn f_random_lane_permutation(
    seed: i64,
    key_mode: KeyMode,
    arrange: ArrangeOption,
    legacy_seed: bool,
) -> Vec<usize> {
    let f_random = shuffle_lane_groups(seed, f_random_lane_groups(key_mode), legacy_seed);
    if arrange == ArrangeOption::MFRandom {
        compose_lane_permutations(&f_random, &mirror_permutation(key_mode))
    } else {
        f_random
    }
}

pub(super) fn f_random_lane_permutation_for_side(
    seed: i64,
    key_mode: KeyMode,
    arrange: ArrangeOption,
    side: ArrangeSide,
    legacy_seed: bool,
) -> Vec<usize> {
    let f_random =
        shuffle_lane_groups(seed, f_random_lane_groups_for_side(key_mode, side), legacy_seed);
    if arrange == ArrangeOption::MFRandom {
        let mirror = mirror_lane_permutation_for_side(key_mode, side);
        compose_lane_permutations(&f_random, &mirror)
    } else {
        f_random
    }
}

pub(super) fn shuffle_lane_groups(
    seed: i64,
    groups: Vec<Vec<usize>>,
    legacy_seed: bool,
) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
    let mut rng = ArrangeRng::new(seed, legacy_seed);
    for group in groups {
        shuffle_lane_group(&mut rng, &group, &mut perm, legacy_seed);
    }
    perm
}

pub(super) fn mirror_lane_permutation_for_side(key_mode: KeyMode, side: ArrangeSide) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
    for group in arrange_lane_groups_for_side(key_mode, false, side) {
        reverse_lane_group(&mut perm, &group);
    }
    perm
}

pub(super) fn compose_lane_permutations(first: &[usize], second: &[usize]) -> Vec<usize> {
    second.iter().map(|&source| first[source]).collect()
}

pub(super) fn rotate_lane_permutation(
    seed: i64,
    key_mode: KeyMode,
    include_scratch: bool,
    legacy_seed: bool,
) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
    let mut rng = ArrangeRng::new(seed, legacy_seed);
    for group in arrange_lane_groups(key_mode, include_scratch) {
        rotate_lane_group(&mut rng, &group, &mut perm);
    }
    perm
}

pub(super) fn rotate_lane_group(rng: &mut ArrangeRng, group: &[usize], perm: &mut [usize]) {
    if group.len() < 2 {
        return;
    }
    let inc = rng.next_usize(2) == 1;
    let mut index = rng.next_usize(group.len() - 1);
    if inc {
        index += 1;
    }
    for &lane in group {
        perm[lane] = group[index];
        index =
            if inc { (index + 1) % group.len() } else { (index + group.len() - 1) % group.len() };
    }
}

pub(super) fn arrange_lane_groups_for_side(
    key_mode: KeyMode,
    include_scratch: bool,
    side: ArrangeSide,
) -> Vec<Vec<usize>> {
    let groups = arrange_lane_groups(key_mode, include_scratch);
    match (key_mode, side) {
        (KeyMode::K10 | KeyMode::K14, ArrangeSide::P1) => groups.into_iter().take(1).collect(),
        (KeyMode::K10 | KeyMode::K14, ArrangeSide::P2) => {
            groups.into_iter().skip(1).take(1).collect()
        }
        (_, ArrangeSide::P1) => groups,
        (_, ArrangeSide::P2) => Vec::new(),
    }
}

pub(super) fn f_random_lane_groups(key_mode: KeyMode) -> Vec<Vec<usize>> {
    arrange_lane_groups(key_mode, false).into_iter().flat_map(split_f_random_group).collect()
}

pub(super) fn f_random_lane_groups_for_side(
    key_mode: KeyMode,
    side: ArrangeSide,
) -> Vec<Vec<usize>> {
    arrange_lane_groups_for_side(key_mode, false, side)
        .into_iter()
        .flat_map(split_f_random_group)
        .collect()
}

pub(super) fn split_f_random_group(group: Vec<usize>) -> Vec<Vec<usize>> {
    let len = group.len();
    if len < 2 {
        return Vec::new();
    }

    let mid = len / 2;
    let mut groups = Vec::with_capacity(2);
    let left = group[..mid].to_vec();
    if left.len() >= 2 {
        groups.push(left);
    }
    let right_start = if len.is_multiple_of(2) { mid } else { mid + 1 };
    let right = group[right_start..].to_vec();
    if right.len() >= 2 {
        groups.push(right);
    }
    groups
}

pub(super) fn arrange_lane_groups(key_mode: KeyMode, include_scratch: bool) -> Vec<Vec<usize>> {
    let active = key_mode.active_lanes();
    match key_mode {
        KeyMode::K4 | KeyMode::K6 | KeyMode::K9 => {
            vec![active.iter().map(|&lane| lane as usize).collect()]
        }
        KeyMode::K5 | KeyMode::K7 | KeyMode::K8 => {
            vec![
                active
                    .iter()
                    .filter(|&&lane| include_scratch || lane != Lane::Scratch)
                    .map(|&lane| lane as usize)
                    .collect(),
            ]
        }
        KeyMode::K10 | KeyMode::K14 => {
            let p1 = active
                .iter()
                .filter(|&&lane| {
                    matches!(
                        lane,
                        Lane::Scratch
                            | Lane::Key1
                            | Lane::Key2
                            | Lane::Key3
                            | Lane::Key4
                            | Lane::Key5
                            | Lane::Key6
                            | Lane::Key7
                    ) && (include_scratch || lane != Lane::Scratch)
                })
                .map(|&lane| lane as usize)
                .collect();
            let p2 = active
                .iter()
                .filter(|&&lane| {
                    matches!(
                        lane,
                        Lane::Key8
                            | Lane::Key9
                            | Lane::Key10
                            | Lane::Key11
                            | Lane::Key12
                            | Lane::Key13
                            | Lane::Key14
                            | Lane::Scratch2
                    ) && (include_scratch || lane != Lane::Scratch2)
                })
                .map(|&lane| lane as usize)
                .collect();
            vec![p1, p2]
        }
    }
}

pub(super) fn reverse_lane_group(perm: &mut [usize], lanes: &[usize]) {
    if lanes.len() < 2 {
        return;
    }
    let reversed: Vec<usize> = lanes.iter().rev().copied().collect();
    for (orig, rev) in lanes.iter().zip(reversed.iter()) {
        perm[*orig] = *rev;
    }
}

pub(super) fn apply_lane_permutation(chart: &mut PlayableChart, perm: &[usize]) {
    let mut old_notes: Vec<Option<Vec<NoteEvent>>> =
        (0..LANE_COUNT).map(|i| Some(std::mem::take(&mut chart.lane_notes[i]))).collect();
    for (new_idx, &old_idx) in perm.iter().enumerate() {
        let new_lane = Lane::ALL[new_idx];
        let notes = old_notes[old_idx].take().unwrap_or_default();
        chart.lane_notes[new_idx] = notes
            .into_iter()
            .map(|mut n| {
                n.lane = new_lane;
                n
            })
            .collect();
    }

    let mut reverse = [0usize; LANE_COUNT];
    for (new_idx, &old_idx) in perm.iter().enumerate() {
        reverse[old_idx] = new_idx;
    }
    for ln in &mut chart.long_notes {
        ln.lane = Lane::ALL[reverse[ln.lane as usize]];
    }
}

pub(super) fn apply_note_arrange(
    chart: &mut PlayableChart,
    arrange: ArrangeOption,
    seed: i64,
    legacy_seed: bool,
) {
    let include_scratch = matches!(arrange, ArrangeOption::AllScratch | ArrangeOption::SRandomEx);
    let groups = arrange_lane_groups(chart.metadata.key_mode, include_scratch);
    apply_note_arrange_for_groups(chart, arrange, seed, &groups, legacy_seed);
}

pub(super) fn apply_note_arrange_for_groups(
    chart: &mut PlayableChart,
    arrange: ArrangeOption,
    seed: i64,
    groups: &[Vec<usize>],
    legacy_seed: bool,
) {
    let mut engine = NoteArrangeEngine::new(arrange, seed, groups, legacy_seed);
    let mut notes: Vec<NoteEvent> = chart.lane_notes.iter_mut().flat_map(std::mem::take).collect();
    notes.sort_by_key(|note| (note.tick, note.time, note.lane as u8, note.id));

    let mut start_to_end = std::collections::HashMap::new();
    let mut end_to_start = std::collections::HashMap::new();
    for ln in &chart.long_notes {
        start_to_end.insert(ln.start_note_id, ln.end_note_id);
        end_to_start.insert(ln.end_note_id, ln.start_note_id);
    }

    let mut arranged = Vec::with_capacity(notes.len());
    let mut index = 0;
    while index < notes.len() {
        let tick = notes[index].tick;
        let mut end = index + 1;
        while end < notes.len() && notes[end].tick == tick {
            end += 1;
        }
        let mut group_notes = notes[index..end].to_vec();
        engine.arrange_timeline(&mut group_notes, &start_to_end, &end_to_start);
        arranged.extend(group_notes);
        index = end;
    }

    for lane_notes in &mut chart.lane_notes {
        lane_notes.clear();
    }
    let mut start_lane = std::collections::HashMap::new();
    for note in arranged {
        if note.kind == NoteKind::LongStart {
            start_lane.insert(note.id, note.lane);
        }
        chart.lane_notes[note.lane.index()].push(note);
    }
    for ln in &mut chart.long_notes {
        if let Some(&lane) = start_lane.get(&ln.start_note_id) {
            ln.lane = lane;
        }
    }
}

pub(super) struct NoteArrangeEngine {
    pub(super) arrange: ArrangeOption,
    pub(super) rng: ArrangeRng,
    pub(super) groups: Vec<NoteArrangeGroup>,
}

impl NoteArrangeEngine {
    pub(super) fn new(
        arrange: ArrangeOption,
        seed: i64,
        groups: &[Vec<usize>],
        legacy_seed: bool,
    ) -> Self {
        Self {
            arrange,
            rng: ArrangeRng::new(seed, legacy_seed),
            groups: groups.iter().map(|lanes| NoteArrangeGroup::new(lanes)).collect(),
        }
    }

    pub(super) fn arrange_timeline(
        &mut self,
        notes: &mut [NoteEvent],
        start_to_end: &std::collections::HashMap<bmz_core::ids::NoteId, bmz_core::ids::NoteId>,
        end_to_start: &std::collections::HashMap<bmz_core::ids::NoteId, bmz_core::ids::NoteId>,
    ) {
        let time = notes.first().map(|note| note.time).unwrap_or(TimeUs(0));
        for group in &mut self.groups {
            let map = group.randomize(notes, time, self.arrange, &mut self.rng);
            for note in notes.iter_mut() {
                let source = note.lane.index();
                let Some(&dest) = map.get(&source) else {
                    continue;
                };
                note.lane = Lane::ALL[dest];
                if note.kind == NoteKind::LongStart {
                    if start_to_end.contains_key(&note.id) {
                        group.active_ln.insert(source, dest);
                    }
                } else if note.kind == NoteKind::LongEnd && end_to_start.contains_key(&note.id) {
                    group.active_ln.remove(&source);
                }
            }
        }
    }
}

pub(super) struct NoteArrangeGroup {
    pub(super) lanes: Vec<usize>,
    pub(super) last_note_time: std::collections::HashMap<usize, TimeUs>,
    pub(super) active_ln: std::collections::HashMap<usize, usize>,
    pub(super) spiral_increment: usize,
    pub(super) spiral_head: usize,
    pub(super) scratch_lanes: Vec<usize>,
    pub(super) scratch_index: usize,
}

impl NoteArrangeGroup {
    pub(super) fn new(lanes: &[usize]) -> Self {
        let scratch_lanes: Vec<usize> = lanes
            .iter()
            .copied()
            .filter(|&lane| lane == Lane::Scratch.index() || lane == Lane::Scratch2.index())
            .collect();
        Self {
            lanes: lanes.to_vec(),
            last_note_time: lanes.iter().copied().map(|lane| (lane, TimeUs(-10_000_000))).collect(),
            active_ln: std::collections::HashMap::new(),
            spiral_increment: 0,
            spiral_head: 0,
            scratch_lanes,
            scratch_index: 0,
        }
    }

    pub(super) fn randomize(
        &mut self,
        notes: &[NoteEvent],
        time: TimeUs,
        arrange: ArrangeOption,
        rng: &mut ArrangeRng,
    ) -> std::collections::HashMap<usize, usize> {
        if self.lanes.is_empty() {
            return std::collections::HashMap::new();
        }
        if arrange == ArrangeOption::Spiral {
            return self.spiral_map(rng);
        }

        let mut changeable = self.changeable_lanes();
        let mut assignable = self.assignable_lanes();
        let mut map = std::collections::HashMap::new();
        map.extend(self.active_ln.iter().map(|(&source, &dest)| (source, dest)));

        if arrange == ArrangeOption::AllScratch {
            self.assign_all_scratch(notes, time, rng, &mut changeable, &mut assignable, &mut map);
        }

        let threshold = match arrange {
            ArrangeOption::SRandom => TimeUs(40_000),
            ArrangeOption::SRandomEx => TimeUs(40_000),
            ArrangeOption::HRandom | ArrangeOption::AllScratch => TimeUs(100_000),
            _ => TimeUs(40_000),
        };
        map.extend(self.time_based_shuffle(notes, time, threshold, rng, changeable, assignable));
        map
    }

    pub(super) fn changeable_lanes(&self) -> Vec<usize> {
        self.lanes.iter().copied().filter(|lane| !self.active_ln.contains_key(lane)).collect()
    }

    pub(super) fn assignable_lanes(&self) -> Vec<usize> {
        self.lanes
            .iter()
            .copied()
            .filter(|lane| !self.active_ln.values().any(|active| active == lane))
            .collect()
    }

    pub(super) fn time_based_shuffle(
        &mut self,
        notes: &[NoteEvent],
        time: TimeUs,
        threshold: TimeUs,
        rng: &mut ArrangeRng,
        changeable: Vec<usize>,
        assignable: Vec<usize>,
    ) -> std::collections::HashMap<usize, usize> {
        let mut note_lane = Vec::new();
        let mut empty_lane = Vec::new();
        for lane in changeable {
            if notes.iter().any(|note| note.lane.index() == lane && note.kind != NoteKind::Mine) {
                note_lane.push(lane);
            } else {
                empty_lane.push(lane);
            }
        }

        let mut primary_lane = Vec::new();
        let mut inferior_lane = Vec::new();
        for lane in assignable {
            let last = self.last_note_time.get(&lane).copied().unwrap_or(TimeUs(-10_000_000));
            if time.0 - last.0 > threshold.0 {
                primary_lane.push(lane);
            } else {
                inferior_lane.push(lane);
            }
        }

        let mut map = std::collections::HashMap::new();
        while !note_lane.is_empty() && !primary_lane.is_empty() {
            let index = rng.next_usize(primary_lane.len());
            map.insert(note_lane.remove(0), primary_lane.remove(index));
        }
        while !note_lane.is_empty() && !inferior_lane.is_empty() {
            let min_time = inferior_lane
                .iter()
                .filter_map(|lane| self.last_note_time.get(lane))
                .map(|time| time.0)
                .min()
                .unwrap_or(-10_000_000);
            let candidates: Vec<usize> = inferior_lane
                .iter()
                .copied()
                .filter(|lane| {
                    self.last_note_time.get(lane).map(|time| time.0).unwrap_or(-10_000_000)
                        == min_time
                })
                .collect();
            let dest = candidates[rng.next_usize(candidates.len())];
            map.insert(note_lane.remove(0), dest);
            inferior_lane.retain(|&lane| lane != dest);
        }

        primary_lane.extend(inferior_lane);
        while !empty_lane.is_empty() && !primary_lane.is_empty() {
            let index = rng.next_usize(primary_lane.len());
            map.insert(empty_lane.remove(0), primary_lane.remove(index));
        }

        for (&source, &dest) in &map {
            if notes.iter().any(|note| note.lane.index() == source && note.kind != NoteKind::Mine) {
                self.last_note_time.insert(dest, time);
            }
        }
        map
    }

    pub(super) fn spiral_map(
        &mut self,
        rng: &mut ArrangeRng,
    ) -> std::collections::HashMap<usize, usize> {
        if self.lanes.len() < 2 {
            return std::collections::HashMap::new();
        }
        if self.spiral_increment == 0 {
            self.spiral_increment = rng.next_usize(self.lanes.len() - 1) + 1;
        }
        let changeable = self.changeable_lanes();
        if changeable.len() == self.lanes.len() {
            self.spiral_head = (self.spiral_head + self.spiral_increment) % self.lanes.len();
        }
        let mut map = std::collections::HashMap::new();
        map.extend(self.active_ln.iter().map(|(&source, &dest)| (source, dest)));
        for (index, &lane) in self.lanes.iter().enumerate() {
            if changeable.contains(&lane) {
                map.insert(lane, self.lanes[(index + self.spiral_head) % self.lanes.len()]);
            }
        }
        map
    }

    pub(super) fn assign_all_scratch(
        &mut self,
        notes: &[NoteEvent],
        time: TimeUs,
        _rng: &mut ArrangeRng,
        changeable: &mut Vec<usize>,
        assignable: &mut Vec<usize>,
        map: &mut std::collections::HashMap<usize, usize>,
    ) {
        if self.scratch_lanes.is_empty() {
            return;
        }
        let scratch = self.scratch_lanes[self.scratch_index];
        let last = self.last_note_time.get(&scratch).copied().unwrap_or(TimeUs(-10_000_000));
        if !assignable.contains(&scratch) || time.0 - last.0 <= 40_000 {
            return;
        }
        let Some(source) = changeable.iter().copied().find(|&lane| {
            notes.iter().any(|note| note.lane.index() == lane && note.kind != NoteKind::Mine)
        }) else {
            return;
        };
        map.insert(source, scratch);
        changeable.retain(|&lane| lane != source);
        assignable.retain(|&lane| lane != scratch);
        self.last_note_time.insert(scratch, time);
        self.scratch_index = (self.scratch_index + 1) % self.scratch_lanes.len();
    }
}

#[derive(Debug, Clone)]
pub(super) struct SplitMix64 {
    pub(super) seed: u64,
}

impl SplitMix64 {
    pub(super) fn new(seed: i64) -> Self {
        Self { seed: seed as u64 }
    }

    pub(super) fn next_u64(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut value = self.seed;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
        value ^ (value >> 31)
    }

    pub(super) fn next_usize(&mut self, bound: usize) -> usize {
        assert!(bound > 0);
        let bound = bound as u128;
        let zone = ((1u128 << 64) / bound) * bound;
        loop {
            let value = self.next_u64() as u128;
            if value < zone {
                return (value % bound) as usize;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum ArrangeRng {
    Beatoraja(JavaRandom),
    Legacy(SplitMix64),
}

impl ArrangeRng {
    pub(super) fn new(seed: i64, legacy_seed: bool) -> Self {
        if legacy_seed {
            Self::Legacy(SplitMix64::new(seed))
        } else {
            Self::Beatoraja(JavaRandom::new(seed))
        }
    }

    pub(super) fn next_usize(&mut self, bound: usize) -> usize {
        assert!(bound > 0);
        match self {
            Self::Beatoraja(random) => random.next_int_bound(bound as i32) as usize,
            Self::Legacy(random) => random.next_usize(bound),
        }
    }
}

pub(super) fn shuffle_lane_group(
    rng: &mut ArrangeRng,
    lanes: &[usize],
    perm: &mut [usize],
    legacy_seed: bool,
) {
    if legacy_seed {
        fisher_yates_shuffle(rng, lanes, perm);
        return;
    }

    // beatoraja LaneRandomShuffleModifier: source lane order is stable and
    // each destination is selected from, then removed from, the remaining list.
    let mut remaining = lanes.to_vec();
    for &lane in lanes {
        let index = rng.next_usize(remaining.len());
        perm[lane] = remaining.remove(index);
    }
}

pub(super) fn fisher_yates_shuffle(rng: &mut ArrangeRng, lanes: &[usize], perm: &mut [usize]) {
    if lanes.len() < 2 {
        return;
    }
    let mut indices: Vec<usize> = lanes.to_vec();
    for i in (1..indices.len()).rev() {
        let j = rng.next_usize(i + 1);
        indices.swap(i, j);
    }
    for (orig, new_target) in lanes.iter().zip(indices.iter()) {
        perm[*orig] = *new_target;
    }
}
