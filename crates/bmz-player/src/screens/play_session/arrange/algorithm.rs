use super::*;

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
