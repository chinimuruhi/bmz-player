use std::collections::{HashMap, HashSet};

use bmz_chart::model::{LongNoteMode, NoteEvent, NoteKind, PlayableChart};
use bmz_core::ids::NoteId;
use bmz_core::input::{InputEvent, InputKind};
use bmz_core::judge::{Judge, TimingSide};
use bmz_core::lane::{KeyMode, LANE_COUNT, Lane};
use bmz_core::time::TimeUs;

use super::model::{
    ActiveLongNote, JudgeAlgorithm, JudgeOutcome, JudgeWindow, JudgeWindows, JudgementEvent,
    KeySoundEvent, LaneJudgeState, LongNoteEndRef, MineHitEvent, PendingLongRelease,
};
use crate::rule::RuleMode;

#[derive(Debug, Clone)]
pub struct JudgeEngine {
    pub windows: JudgeWindow,
    pub window_set: JudgeWindows,
    pub rule_mode: RuleMode,
    pub algorithm: JudgeAlgorithm,
    pub lanes: [LaneJudgeState; LANE_COUNT],
    pub judged_notes: HashMap<NoteId, Judge>,
    bad_attempted_notes: HashSet<NoteId>,
    bad_judge_vanish: bool,
}

impl JudgeEngine {
    pub fn new(windows: JudgeWindow) -> Self {
        Self::new_with_rule_mode(windows, RuleMode::Beatoraja)
    }

    pub fn new_with_rule_mode(windows: JudgeWindow, rule_mode: RuleMode) -> Self {
        Self::new_with_window_set(JudgeWindows::uniform(windows), rule_mode)
    }

    pub fn new_with_window_set(window_set: JudgeWindows, rule_mode: RuleMode) -> Self {
        Self::new_with_window_set_and_algorithm(window_set, rule_mode, JudgeAlgorithm::Combo)
    }

    pub fn new_with_window_set_and_algorithm(
        window_set: JudgeWindows,
        rule_mode: RuleMode,
        algorithm: JudgeAlgorithm,
    ) -> Self {
        Self::new_with_window_set_algorithm_and_keymode(
            window_set,
            rule_mode,
            algorithm,
            KeyMode::K7,
        )
    }

    pub fn new_with_window_set_algorithm_and_keymode(
        window_set: JudgeWindows,
        rule_mode: RuleMode,
        algorithm: JudgeAlgorithm,
        key_mode: KeyMode,
    ) -> Self {
        Self {
            windows: window_set.note,
            window_set,
            rule_mode,
            algorithm,
            lanes: [LaneJudgeState::default(); LANE_COUNT],
            judged_notes: HashMap::new(),
            bad_attempted_notes: HashSet::new(),
            bad_judge_vanish: bad_judge_vanish_for_keymode_and_rule_mode(key_mode, rule_mode),
        }
    }

    pub fn set_window_set(&mut self, window_set: JudgeWindows) {
        self.windows = window_set.note;
        self.window_set = window_set;
    }

    pub fn process_input(&mut self, chart: &PlayableChart, input: InputEvent) -> JudgeOutcome {
        match input.kind {
            InputKind::Press => self.process_press(chart, input),
            InputKind::Release => self.process_release(chart, input),
        }
    }

    pub fn process_misses(&mut self, chart: &PlayableChart, now: TimeUs) -> JudgeOutcome {
        let mut outcome = JudgeOutcome::default();

        for lane in Lane::ALL {
            let lane_state = &mut self.lanes[lane.index()];

            while let Some((idx, note)) = next_unjudged_press_reference_note(
                chart,
                lane,
                lane_state.next_note_index,
                &self.judged_notes,
            ) {
                let windows = self.window_set.press_window(lane);
                if now.0 <= note.time.0 + windows.bad_slow_us {
                    break;
                }

                lane_state.next_note_index = idx + 1;
                let bad_was_already_scored = self.bad_attempted_notes.remove(&note.id);
                self.judged_notes.insert(note.id, Judge::Poor);
                let miss_delta = TimeUs(now.0 - note.time.0);
                if !bad_was_already_scored {
                    outcome.events.push(JudgementEvent {
                        note_id: Some(note.id),
                        lane,
                        judge: Judge::Poor,
                        side: TimingSide::Slow,
                        delta: miss_delta,
                        time: now,
                        affects_score: true,
                    });
                }
                // beatoraja treats a missed CN/HCN head as two misses: both the
                // head and its paired tail become POOR immediately. If the head
                // had already produced a non-vanishing BAD, only the tail adds
                // another score event (MissCondition.ONE).
                if let Some(end_note_id) = missed_charge_end_for_start(chart, note.id) {
                    self.judged_notes.insert(end_note_id, Judge::Poor);
                    outcome.events.push(JudgementEvent {
                        note_id: Some(end_note_id),
                        lane,
                        judge: Judge::Poor,
                        side: TimingSide::Slow,
                        delta: miss_delta,
                        time: now,
                        affects_score: true,
                    });
                }
            }
            advance_press_cursor(chart, lane, &mut lane_state.next_note_index, &self.judged_notes);

            if let Some(active) = lane_state.active_long {
                let release_margin_us = self.window_set.long_release_margin_us(lane);
                if let Some(pending) = active.pending_release
                    && now.0 >= pending.released_at.0 + release_margin_us
                {
                    lane_state.active_long = None;
                    let judged_note_id = active_scored_note_id(active);
                    self.judged_notes.insert(judged_note_id, pending.judge);
                    append_outcome(
                        &mut outcome,
                        finalize_long_release(
                            chart,
                            lane,
                            active,
                            pending.judge,
                            pending.delta,
                            now,
                        ),
                    );
                    continue;
                }

                match active.mode {
                    LongNoteMode::Ln => {
                        if now.0 > active.end.end_time.0 {
                            lane_state.active_long = None;
                            self.judged_notes.insert(active.start_note_id, active.start_judge);
                            outcome.events.push(ln_final_event(
                                lane,
                                active,
                                active.start_judge,
                                active.start_delta,
                                now,
                            ));
                            outcome
                                .keysounds
                                .push(KeySoundEvent { note_id: active.end.end_note_id, time: now });
                        }
                    }
                    LongNoteMode::Cn | LongNoteMode::Hcn => {
                        // beatoraja uses the normal note BAD-late boundary for
                        // an unreleased CN/HCN tail. The long-end window is only
                        // used when an actual Release input occurs.
                        let windows = self.window_set.press_window(lane);
                        if now.0 > active.end.end_time.0 + windows.bad_slow_us {
                            lane_state.active_long = None;
                            self.judged_notes.insert(active.end.end_note_id, Judge::Poor);
                            outcome.events.push(JudgementEvent {
                                note_id: Some(active.end.end_note_id),
                                lane,
                                judge: Judge::Poor,
                                side: TimingSide::Slow,
                                delta: TimeUs(now.0 - active.end.end_time.0),
                                time: now,
                                affects_score: true,
                            });
                        }
                    }
                }
            }
        }

        outcome
    }

    pub fn process_mine_passes(
        &mut self,
        chart: &PlayableChart,
        now: TimeUs,
        lane_keyon_started_at: &[Option<TimeUs>; LANE_COUNT],
    ) -> JudgeOutcome {
        let mut outcome = JudgeOutcome::default();

        for lane in Lane::ALL {
            let lane_index = lane.index();
            let lane_state = &mut self.lanes[lane.index()];
            let notes = chart.notes_for_lane(lane);
            while let Some(note) = notes.get(lane_state.next_mine_index) {
                if note.time > now {
                    break;
                }
                lane_state.next_mine_index += 1;
                let Some(keyon_started_at) = lane_keyon_started_at[lane_index] else {
                    continue;
                };
                if note.kind != NoteKind::Mine
                    || keyon_started_at > note.time
                    || Some(note.time) == lane_state.last_mine_hit_time
                {
                    continue;
                }

                lane_state.last_mine_hit_time = Some(note.time);
                outcome.mine_hits.push(MineHitEvent {
                    note_id: note.id,
                    lane,
                    damage: note.damage.unwrap_or(0),
                    time: note.time,
                });
            }
        }

        outcome
    }

    pub fn is_exhausted(&self, chart: &PlayableChart) -> bool {
        Lane::ALL.iter().copied().all(|lane| {
            let state = &self.lanes[lane.index()];
            state.active_long.is_none()
                && next_unjudged_press_reference_note(
                    chart,
                    lane,
                    state.next_note_index,
                    &self.judged_notes,
                )
                .is_none()
        })
    }

    fn process_press(&mut self, chart: &PlayableChart, input: InputEvent) -> JudgeOutcome {
        // Mine ヒット判定は通常ノーツの判定に先んじて、もしくは並走して行う。
        // 入力は通常ノーツの判定を妨げないので、ここでは別ベクタに積むだけ。
        let mut mine_hits = Vec::new();
        if let Some(hit) = detect_mine_hit(
            chart,
            input.lane,
            input.time,
            self.window_set.press_window(input.lane).mine_hit_us,
            &self.lanes[input.lane.index()],
        ) {
            self.lanes[input.lane.index()].last_mine_hit_time = Some(hit.time);
            mine_hits.push(hit);
        }

        if let Some(mut active) = self.lanes[input.lane.index()].active_long {
            if active.pending_release.take().is_some() {
                self.lanes[input.lane.index()].active_long = Some(active);
                return JudgeOutcome { mine_hits, consumed_input: true, ..Default::default() };
            }
            return JudgeOutcome { mine_hits, ..Default::default() };
        }

        let rule_mode = self.rule_mode;
        let windows = self.window_set.press_window(input.lane);
        let candidate = select_press_candidate(
            chart,
            input.lane,
            input.time,
            windows,
            rule_mode,
            self.algorithm,
            &self.judged_notes,
            &self.bad_attempted_notes,
        );
        let Some(candidate) = candidate else {
            return JudgeOutcome { mine_hits, ..Default::default() };
        };

        if candidate.consumes_note {
            // candidate 生成側の不変条件が崩れてもプレイ中に panic せず、
            // その入力の判定だけを捨てる (debug build では検知する)。
            let Some(note_id) = candidate.note_id else {
                debug_assert!(false, "normal candidate must have note id");
                return JudgeOutcome { mine_hits, ..Default::default() };
            };
            let Some(note) = chart.note_by_id(note_id) else {
                debug_assert!(false, "candidate note {note_id:?} must exist in chart");
                return JudgeOutcome { mine_hits, ..Default::default() };
            };
            let note_vanishes = candidate.judge != Judge::Bad || self.bad_judge_vanish;
            let multi_bad_candidates = if matches!(rule_mode, RuleMode::Lr2Oraja | RuleMode::Dx) {
                lr2oraja_multi_bad_candidates(
                    chart,
                    input.lane,
                    input.time,
                    windows,
                    note,
                    candidate,
                    &self.judged_notes,
                )
            } else {
                Vec::new()
            };

            let lane_state = &mut self.lanes[input.lane.index()];
            lane_state.last_press_time = Some(note.time);
            for multi_bad in &multi_bad_candidates {
                if self.bad_judge_vanish {
                    self.judged_notes.insert(multi_bad.note_id, Judge::Bad);
                } else {
                    self.bad_attempted_notes.insert(multi_bad.note_id);
                }
            }
            if note_vanishes {
                self.bad_attempted_notes.remove(&note.id);
                self.judged_notes.insert(note.id, candidate.judge);
            } else {
                self.bad_attempted_notes.insert(note.id);
            }

            if note_vanishes
                && note.kind == NoteKind::LongStart
                && let Some(active) =
                    make_active_long(chart, note.id, candidate.judge, candidate.delta, input.time)
            {
                lane_state.active_long = Some(active);
            }
            advance_press_cursor(
                chart,
                input.lane,
                &mut lane_state.next_note_index,
                &self.judged_notes,
            );

            let mut events = Vec::with_capacity(multi_bad_candidates.len() + 1);
            events.extend(multi_bad_candidates.into_iter().map(|multi_bad| JudgementEvent {
                note_id: Some(multi_bad.note_id),
                lane: input.lane,
                judge: Judge::Bad,
                side: side_from_delta(multi_bad.delta.0),
                delta: multi_bad.delta,
                time: input.time,
                affects_score: true,
            }));
            events.push(JudgementEvent {
                note_id: Some(note_id),
                lane: input.lane,
                judge: candidate.judge,
                side: candidate.side,
                delta: candidate.delta,
                time: input.time,
                affects_score: note.kind != NoteKind::LongStart
                    || active_long_scores_on_start(chart, note.id),
            });

            return JudgeOutcome {
                events,
                keysounds: vec![KeySoundEvent { note_id, time: input.time }],
                mine_hits,
                consumed_input: true,
                ..Default::default()
            };
        }

        let Some(keysound_note_id) = candidate.keysound_note_id else {
            debug_assert!(false, "empty poor candidate must have key sound note id");
            return JudgeOutcome { mine_hits, ..Default::default() };
        };
        let mut outcome =
            empty_poor(input.lane, candidate.side, candidate.delta, input.time, keysound_note_id);
        outcome.mine_hits = mine_hits;
        outcome
    }

    fn process_release(&mut self, chart: &PlayableChart, input: InputEvent) -> JudgeOutcome {
        let lane_state = &mut self.lanes[input.lane.index()];
        let Some(mut active) = lane_state.active_long else {
            return JudgeOutcome::default();
        };
        let release_margin_us = self.window_set.long_release_margin_us(input.lane);

        match active.mode {
            LongNoteMode::Ln => {
                let end_delta = TimeUs(input.time.0 - active.end.end_time.0);
                let (judge, delta) = if end_delta.0 >= 0 {
                    (active.start_judge, active.start_delta)
                } else {
                    let windows = self.window_set.long_end_window(input.lane);
                    let end_judge =
                        classify_normal_delta(end_delta.0, windows).unwrap_or(Judge::Poor);
                    combine_ln_judgement(active, end_judge, end_delta)
                };
                if release_margin_us > 0
                    && end_delta.0 < 0
                    && matches!(judge, Judge::Bad | Judge::Poor)
                {
                    active.pending_release =
                        Some(PendingLongRelease { released_at: input.time, judge, delta });
                    lane_state.active_long = Some(active);
                    return JudgeOutcome { consumed_input: true, ..Default::default() };
                }
                lane_state.active_long = None;
                self.judged_notes.insert(active.start_note_id, judge);
                finalize_long_release(chart, input.lane, active, judge, delta, input.time)
            }
            LongNoteMode::Cn | LongNoteMode::Hcn => {
                let delta = input.time.0 - active.end.end_time.0;
                let windows = self.window_set.long_end_window(input.lane);
                let judge = classify_normal_delta(delta, windows).unwrap_or(Judge::Poor);
                if release_margin_us > 0 && delta < 0 && matches!(judge, Judge::Bad | Judge::Poor) {
                    active.pending_release = Some(PendingLongRelease {
                        released_at: input.time,
                        judge,
                        delta: TimeUs(delta),
                    });
                    lane_state.active_long = Some(active);
                    return JudgeOutcome { consumed_input: true, ..Default::default() };
                }
                lane_state.active_long = None;
                self.judged_notes.insert(active.end.end_note_id, judge);
                finalize_long_release(chart, input.lane, active, judge, TimeUs(delta), input.time)
            }
        }
    }
}

fn push_early_bad_long_start_mute(
    chart: &PlayableChart,
    active: ActiveLongNote,
    judge: Judge,
    end_delta: TimeUs,
    outcome: &mut JudgeOutcome,
) {
    if end_delta.0 < 0
        && matches!(judge, Judge::Bad | Judge::Poor)
        && let Some(sound_id) = chart.long_notes.get(active.pair_index).and_then(|pair| pair.sound)
    {
        outcome.keysound_volumes.push((sound_id, 0.0));
    }
}

fn finalize_long_release(
    chart: &PlayableChart,
    lane: Lane,
    active: ActiveLongNote,
    judge: Judge,
    delta: TimeUs,
    time: TimeUs,
) -> JudgeOutcome {
    let mut outcome = match active.mode {
        LongNoteMode::Ln => JudgeOutcome {
            events: vec![ln_final_event(lane, active, judge, delta, time)],
            keysounds: vec![KeySoundEvent { note_id: active.end.end_note_id, time }],
            consumed_input: true,
            ..Default::default()
        },
        LongNoteMode::Cn | LongNoteMode::Hcn => JudgeOutcome {
            events: vec![JudgementEvent {
                note_id: Some(active.end.end_note_id),
                lane,
                judge,
                side: side_from_delta(delta.0),
                delta,
                time,
                affects_score: true,
            }],
            keysounds: vec![KeySoundEvent { note_id: active.end.end_note_id, time }],
            consumed_input: true,
            ..Default::default()
        },
    };
    if active.mode != LongNoteMode::Hcn {
        push_early_bad_long_start_mute(chart, active, judge, delta, &mut outcome);
    }
    outcome
}

fn append_outcome(target: &mut JudgeOutcome, mut source: JudgeOutcome) {
    target.events.append(&mut source.events);
    target.keysounds.append(&mut source.keysounds);
    target.keysound_volumes.append(&mut source.keysound_volumes);
    target.consumed_input |= source.consumed_input;
}

fn active_scored_note_id(active: ActiveLongNote) -> NoteId {
    match active.mode {
        LongNoteMode::Ln => active.start_note_id,
        LongNoteMode::Cn | LongNoteMode::Hcn => active.end.end_note_id,
    }
}

fn suppresses_long_start_late_bad(
    rule_mode: RuleMode,
    windows: JudgeWindow,
    note: &NoteEvent,
    delta: i64,
    judge: Judge,
) -> bool {
    matches!(rule_mode, RuleMode::Lr2Oraja | RuleMode::Dx)
        && note.kind == NoteKind::LongStart
        && judge == Judge::Bad
        && delta > windows.good_us
}

#[derive(Debug, Clone, Copy)]
struct PressCandidate {
    note_id: Option<NoteId>,
    keysound_note_id: Option<NoteId>,
    judge: Judge,
    side: TimingSide,
    delta: TimeUs,
    consumes_note: bool,
}

#[derive(Debug, Clone, Copy)]
struct MultiBadCandidate {
    note_id: NoteId,
    note_kind: NoteKind,
    delta: TimeUs,
}

fn select_press_candidate(
    chart: &PlayableChart,
    lane: Lane,
    input_time: TimeUs,
    windows: JudgeWindow,
    rule_mode: RuleMode,
    algorithm: JudgeAlgorithm,
    judged_notes: &HashMap<NoteId, Judge>,
    bad_attempted_notes: &HashSet<NoteId>,
) -> Option<PressCandidate> {
    let mut normal: Option<PressCandidate> = None;
    let mut slow_empty_poor: Option<PressCandidate> = None;
    let mut fast_empty_poor: Option<PressCandidate> = None;
    let scan_fast_us = windows.bad_fast_us.max(windows.empty_poor_fast_us);
    let scan_slow_us = windows.bad_slow_us.max(windows.empty_poor_slow_us);

    for note in chart.notes_for_lane(lane) {
        if note.time.0 - input_time.0 > scan_fast_us {
            break;
        }
        if input_time.0 - note.time.0 > scan_slow_us || !is_press_reference_note(note) {
            continue;
        }

        let delta = input_time.0 - note.time.0;
        let already_judged = judged_notes.contains_key(&note.id);
        let bad_attempted = bad_attempted_notes.contains(&note.id);
        if !already_judged
            && let Some(judge) = classify_normal_delta(delta, windows).filter(|judge| {
                !suppresses_long_start_late_bad(rule_mode, windows, note, delta, *judge)
            })
        {
            if bad_attempted && judge == Judge::Bad {
                continue;
            }
            let candidate = PressCandidate {
                note_id: Some(note.id),
                keysound_note_id: Some(note.id),
                judge,
                side: side_from_delta(delta),
                delta: TimeUs(delta),
                consumes_note: true,
            };
            if normal.as_ref().is_none_or(|current| {
                judge_algorithm_prefers_new_candidate(algorithm, *current, candidate, windows)
            }) {
                normal = Some(candidate);
            }
            continue;
        }

        if bad_attempted {
            continue;
        }

        let empty_poor_candidate = if already_judged {
            if delta >= 0 && delta <= windows.empty_poor_slow_us {
                Some(PressCandidate {
                    note_id: None,
                    keysound_note_id: Some(note.id),
                    judge: Judge::EmptyPoor,
                    side: TimingSide::Slow,
                    delta: TimeUs(delta),
                    consumes_note: false,
                })
            } else if delta < 0 && -delta <= windows.empty_poor_fast_us {
                Some(PressCandidate {
                    note_id: None,
                    keysound_note_id: Some(note.id),
                    judge: Judge::EmptyPoor,
                    side: TimingSide::Fast,
                    delta: TimeUs(delta),
                    consumes_note: false,
                })
            } else {
                None
            }
        } else if delta > windows.bad_slow_us && delta <= windows.empty_poor_slow_us {
            Some(PressCandidate {
                note_id: None,
                keysound_note_id: Some(note.id),
                judge: Judge::EmptyPoor,
                side: TimingSide::Slow,
                delta: TimeUs(delta),
                consumes_note: false,
            })
        } else if delta < -windows.bad_fast_us && -delta <= windows.empty_poor_fast_us {
            Some(PressCandidate {
                note_id: None,
                keysound_note_id: Some(note.id),
                judge: Judge::EmptyPoor,
                side: TimingSide::Fast,
                delta: TimeUs(delta),
                consumes_note: false,
            })
        } else {
            None
        };

        let Some(candidate) = empty_poor_candidate else {
            continue;
        };
        match candidate.side {
            TimingSide::Slow => choose_closest_empty_poor(&mut slow_empty_poor, candidate),
            TimingSide::Fast => choose_closest_empty_poor(&mut fast_empty_poor, candidate),
        }
    }

    normal.or(slow_empty_poor).or(fast_empty_poor)
}

fn judge_algorithm_prefers_new_candidate(
    algorithm: JudgeAlgorithm,
    current: PressCandidate,
    candidate: PressCandidate,
    windows: JudgeWindow,
) -> bool {
    match algorithm {
        JudgeAlgorithm::Combo => {
            current.delta.0 > windows.good_us && candidate.delta.0 >= -windows.good_us
        }
        JudgeAlgorithm::Duration => candidate.delta.0.abs() < current.delta.0.abs(),
        JudgeAlgorithm::Lowest => false,
        JudgeAlgorithm::Score => {
            current.delta.0 > windows.great_us && candidate.delta.0 >= -windows.great_us
        }
    }
}

fn choose_closest_empty_poor(slot: &mut Option<PressCandidate>, candidate: PressCandidate) {
    if slot.as_ref().is_none_or(|current| candidate.delta.0.abs() < current.delta.0.abs()) {
        *slot = Some(candidate);
    }
}

fn lr2oraja_multi_bad_candidates(
    chart: &PlayableChart,
    lane: Lane,
    input_time: TimeUs,
    windows: JudgeWindow,
    selected_note: &NoteEvent,
    selected_candidate: PressCandidate,
    judged_notes: &HashMap<NoteId, Judge>,
) -> Vec<MultiBadCandidate> {
    let selected_dmtime = -selected_candidate.delta.0;
    let mut candidates = chart
        .notes_for_lane(lane)
        .iter()
        .take_while(|note| note.time.0 - input_time.0 <= windows.bad_fast_us)
        .filter(|note| {
            is_press_reference_note(note)
                && note.id != selected_note.id
                && !judged_notes.contains_key(&note.id)
        })
        .filter_map(|note| {
            let delta = input_time.0 - note.time.0;
            (in_bad_range(delta, windows) && !in_good_range(delta, windows)).then_some(
                MultiBadCandidate { note_id: note.id, note_kind: note.kind, delta: TimeUs(delta) },
            )
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|candidate| -candidate.delta.0);

    if selected_candidate.judge != Judge::Bad || selected_note.kind == NoteKind::LongStart {
        candidates.retain(|candidate| -candidate.delta.0 < selected_dmtime);
    }

    let array_start = candidates
        .iter()
        .position(|candidate| {
            -candidate.delta.0 >= selected_dmtime || candidate.note_kind != NoteKind::LongStart
        })
        .unwrap_or(candidates.len());
    candidates.into_iter().skip(array_start).collect()
}

fn combine_ln_judgement(
    active: ActiveLongNote,
    end_judge: Judge,
    end_delta: TimeUs,
) -> (Judge, TimeUs) {
    let mut judge = worse_judge(active.start_judge, end_judge);
    let mut delta =
        if active.start_delta.0.abs() > end_delta.0.abs() { active.start_delta } else { end_delta };

    if end_delta.0 < 0 && matches!(judge, Judge::Bad | Judge::Poor) {
        judge = Judge::Bad;
        delta = end_delta;
    }

    (judge, delta)
}

fn worse_judge(left: Judge, right: Judge) -> Judge {
    if judge_order(left) >= judge_order(right) { left } else { right }
}

fn judge_order(judge: Judge) -> u8 {
    match judge {
        Judge::PGreat => 0,
        Judge::Great => 1,
        Judge::Good => 2,
        Judge::Bad => 3,
        Judge::Poor => 4,
        Judge::EmptyPoor => 5,
    }
}

fn next_unjudged_press_reference_note<'a>(
    chart: &'a PlayableChart,
    lane: Lane,
    start_index: usize,
    judged_notes: &HashMap<NoteId, Judge>,
) -> Option<(usize, &'a NoteEvent)> {
    chart
        .notes_for_lane(lane)
        .iter()
        .enumerate()
        .skip(start_index)
        .find(|(_, note)| is_press_reference_note(note) && !judged_notes.contains_key(&note.id))
}

fn advance_press_cursor(
    chart: &PlayableChart,
    lane: Lane,
    next_note_index: &mut usize,
    judged_notes: &HashMap<NoteId, Judge>,
) {
    let notes = chart.notes_for_lane(lane);
    while let Some(note) = notes.get(*next_note_index) {
        if is_press_reference_note(note) && !judged_notes.contains_key(&note.id) {
            break;
        }
        *next_note_index += 1;
    }
}

fn is_press_reference_note(note: &NoteEvent) -> bool {
    matches!(note.kind, NoteKind::Tap | NoteKind::LongStart)
}

/// 指定レーンに置かれた Mine の中から、入力時刻と `window_us` 以内に一致するものを探す。
/// 直近に同じ time の Mine をヒット済みなら無視する（二重ヒット防止）。
fn detect_mine_hit(
    chart: &PlayableChart,
    lane: Lane,
    input_time: TimeUs,
    window_us: i64,
    lane_state: &LaneJudgeState,
) -> Option<MineHitEvent> {
    chart
        .notes_for_lane(lane)
        .iter()
        .filter(|note| note.kind == NoteKind::Mine)
        .filter(|note| Some(note.time) != lane_state.last_mine_hit_time)
        .find(|note| (input_time.0 - note.time.0).abs() <= window_us)
        .map(|note| MineHitEvent {
            note_id: note.id,
            lane,
            damage: note.damage.unwrap_or(0),
            time: note.time,
        })
}

fn classify_normal_delta(delta_us: i64, windows: JudgeWindow) -> Option<Judge> {
    let abs = delta_us.abs();

    if abs <= windows.pgreat_us {
        Some(Judge::PGreat)
    } else if abs <= windows.great_us {
        Some(Judge::Great)
    } else if abs <= windows.good_us {
        Some(Judge::Good)
    } else if (delta_us < 0 && abs <= windows.bad_fast_us)
        || (delta_us >= 0 && abs <= windows.bad_slow_us)
    {
        Some(Judge::Bad)
    } else {
        None
    }
}

fn in_good_range(delta_us: i64, windows: JudgeWindow) -> bool {
    delta_us.abs() <= windows.good_us
}

fn in_bad_range(delta_us: i64, windows: JudgeWindow) -> bool {
    (delta_us < 0 && -delta_us <= windows.bad_fast_us)
        || (delta_us >= 0 && delta_us <= windows.bad_slow_us)
}

fn bad_judge_vanish_for_keymode_and_rule_mode(key_mode: KeyMode, rule_mode: RuleMode) -> bool {
    !(matches!(rule_mode, RuleMode::Beatoraja | RuleMode::Dx) && key_mode == KeyMode::K9)
}

fn side_from_delta(delta_us: i64) -> TimingSide {
    if delta_us < 0 { TimingSide::Fast } else { TimingSide::Slow }
}

fn make_active_long(
    chart: &PlayableChart,
    start_note_id: NoteId,
    start_judge: Judge,
    start_delta: TimeUs,
    started_at: TimeUs,
) -> Option<ActiveLongNote> {
    let (pair_index, pair) = chart
        .long_notes
        .iter()
        .enumerate()
        .find(|(_, pair)| pair.start_note_id == start_note_id)?;

    Some(ActiveLongNote {
        pair_index,
        mode: pair.mode.unwrap_or(chart.metadata.long_note_mode),
        start_note_id,
        start_judge,
        start_delta,
        end: LongNoteEndRef {
            end_note_id: pair.end_note_id,
            end_tick: pair.end_tick,
            end_time: pair.end_time,
        },
        started_at,
        pending_release: None,
    })
}

fn missed_charge_end_for_start(chart: &PlayableChart, start_note_id: NoteId) -> Option<NoteId> {
    chart.long_notes.iter().find_map(|pair| {
        let mode = pair.mode.unwrap_or(chart.metadata.long_note_mode);
        (pair.start_note_id == start_note_id
            && matches!(mode, LongNoteMode::Cn | LongNoteMode::Hcn))
        .then_some(pair.end_note_id)
    })
}

fn active_long_scores_on_start(chart: &PlayableChart, start_note_id: NoteId) -> bool {
    chart
        .long_notes
        .iter()
        .find(|pair| pair.start_note_id == start_note_id)
        .map(|pair| pair.mode.unwrap_or(chart.metadata.long_note_mode) != LongNoteMode::Ln)
        .unwrap_or(true)
}

fn ln_final_event(
    lane: Lane,
    active: ActiveLongNote,
    judge: Judge,
    delta: TimeUs,
    time: TimeUs,
) -> JudgementEvent {
    JudgementEvent {
        note_id: Some(active.start_note_id),
        lane,
        judge,
        side: side_from_delta(delta.0),
        delta,
        time,
        affects_score: true,
    }
}

fn empty_poor(
    lane: Lane,
    side: TimingSide,
    delta: TimeUs,
    time: TimeUs,
    keysound_note_id: NoteId,
) -> JudgeOutcome {
    JudgeOutcome {
        events: vec![JudgementEvent {
            note_id: None,
            lane,
            judge: Judge::EmptyPoor,
            side,
            delta,
            time,
            affects_score: true,
        }],
        keysounds: vec![KeySoundEvent { note_id: keysound_note_id, time }],
        mine_hits: Vec::new(),
        consumed_input: false,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "engine/tests.rs"]
mod tests;
