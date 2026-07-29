pub fn apply_judge_outcome(
    session: &mut GameSession,
    mut outcome: JudgeOutcome,
) -> Vec<JudgementEvent> {
    let has_display_only_event =
        outcome.events.iter().any(|event| session.display_only_lane_mask[event.lane.index()]);
    for event in &mut outcome.events {
        if session.display_only_lane_mask[event.lane.index()] {
            if event.affects_score {
                if let Some(score) = &mut session.opponent_score {
                    score.apply(event);
                }
                if let Some(gauge) = &mut session.opponent_gauge {
                    let previous_gauge = gauge.current().value;
                    gauge.apply_judge(event.judge, 1.0);
                    if gauge.current().value > previous_gauge + f32::EPSILON {
                        session.opponent_gauge_increase_started_at =
                            Some(TimeUs(event.time.0.max(0)));
                    }
                }
            }
            event.affects_score = false;
        }
    }
    outcome.mine_hits.retain(|hit| {
        if session.display_only_lane_mask[hit.lane.index()] {
            if let Some(gauge) = &mut session.opponent_gauge {
                gauge.apply_mine(hit.damage);
            }
            false
        } else {
            true
        }
    });
    if has_display_only_event {
        // HCN の早離しに伴う音量変更も共有 sound id へ作用するため、表示専用の
        // opponent レーンから発生したものは primary の音声へ反映しない。
        outcome.keysound_volumes.clear();
    }
    outcome.keysounds.retain(|keysound| {
        session
            .chart
            .note_by_id(keysound.note_id)
            .is_none_or(|note| !session.display_only_lane_mask[note.lane.index()])
    });
    let mut events = Vec::with_capacity(outcome.events.len());
    for event in outcome.events {
        if event.affects_score {
            session.score.apply(&event);
            update_course_combo_state(session, &event);
            let previous_gauge = session.gauge.current().value;
            session.gauge.apply_judge(event.judge, 1.0);
            update_gauge_increase_timer(session, previous_gauge, event.time);
            if let Some(note_id) = event.note_id {
                session.result_judgements.insert(
                    note_id,
                    ResultJudgementDetail {
                        judge: event.judge,
                        side: event.side,
                        delta: event.delta,
                        time: event.time,
                    },
                );
            }
        }
        push_skin_runtime_event(session, SkinRuntimeEventKind::Judgement(event.clone()));
        events.push(event);
    }
    for hit in outcome.mine_hits {
        // Mine はスコア/コンボに影響を与えず、ゲージのみ削る。SE 再生 (= app 層の
        // 副作用) は `pending_mine_hits` に積んでフレーム終端で吸い出す。
        session.gauge.apply_mine(hit.damage);
        session.pending_mine_hits.push(hit);
    }
    session.pending_keysounds.extend(outcome.keysounds);
    session.pending_keysound_volumes.extend(outcome.keysound_volumes);
    update_failed_state_from_gauge(session);
    events
}

fn push_skin_runtime_event(session: &mut GameSession, kind: SkinRuntimeEventKind) {
    let sequence = session.next_skin_event_sequence;
    session.next_skin_event_sequence = session
        .next_skin_event_sequence
        .checked_add(1)
        .expect("skin runtime event sequence exhausted");
    session.pending_skin_events.push(SkinRuntimeEvent { sequence, kind });
}

impl GameSession {
    pub fn display_combo(&self) -> u32 {
        if self.course_combo_carry_active {
            self.course_combo_carry.saturating_add(self.score.combo)
        } else {
            self.score.combo
        }
    }

    pub fn display_max_combo(&self) -> u32 {
        self.course_max_combo.max(self.score.max_combo)
    }
}

fn update_course_combo_state(session: &mut GameSession, event: &JudgementEvent) {
    match event.judge {
        Judge::PGreat | Judge::Great | Judge::Good => {
            session.course_max_combo = session.course_max_combo.max(session.display_combo());
        }
        Judge::Bad | Judge::Poor => {
            session.course_combo_carry_active = false;
        }
        Judge::EmptyPoor if session.score.empty_poor_breaks_combo => {
            session.course_combo_carry_active = false;
        }
        Judge::EmptyPoor => {}
    }
}

fn update_failed_state_from_gauge(session: &mut GameSession) {
    if session.state == PlayState::Playing && session.gauge.current_closes_play_on_zero() {
        session.state = PlayState::Failed;
    }
}

fn update_gauge_increase_timer(session: &mut GameSession, previous_value: f32, now: TimeUs) {
    let current_value = session.gauge.current().value;
    if current_value > previous_value + f32::EPSILON {
        session.gauge_increase_started_at = Some(TimeUs(now.0.max(0)));
    }
}

fn update_gauge_max_timer(session: &mut GameSession, now: TimeUs) {
    let current = session.gauge.current();
    let is_max = current.value >= current.definition.max.max(1.0);
    match (is_max, session.gauge_max_started_at) {
        (true, None) => session.gauge_max_started_at = Some(TimeUs(now.0.max(0))),
        (false, Some(_)) => session.gauge_max_started_at = None,
        _ => {}
    }
    if let Some(opponent_gauge) = &session.opponent_gauge {
        let is_max =
            opponent_gauge.current().value >= opponent_gauge.current().definition.max.max(1.0);
        match (is_max, session.opponent_gauge_max_started_at) {
            (true, None) => session.opponent_gauge_max_started_at = Some(TimeUs(now.0.max(0))),
            (false, Some(_)) => session.opponent_gauge_max_started_at = None,
            _ => {}
        }
    }
}
