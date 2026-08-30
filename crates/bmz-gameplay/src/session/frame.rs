pub fn sync_judge_windows(session: &mut GameSession, now: TimeUs) {
    let percent = judge_percent_at_time_for_keymode(
        session.chart.metadata.judge_rank_spec,
        &session.chart.judge_rank_events,
        now,
        session.primary_key_mode,
        session.rule_mode,
    );
    let windows = judge_windows_for_rule_mode_and_keymode(
        session.base_judge_windows,
        percent,
        session.rule_mode,
        session.primary_key_mode,
    );
    session.judge.set_window_set(scale_judge_windows_for_playback_rate(
        windows,
        session.audio_clock.playback_rate_percent(),
    ));
}

use super::judgement::{update_failed_state_from_gauge, update_gauge_max_timer};

pub(super) fn sync_input_timestamp_anchor(session: &mut GameSession, audio_now: TimeUs) {
    session.input_timestamp_anchor = if session.audio_clock.running {
        Some(InputTimestampAnchor { monotonic_ns: monotonic_timestamp_ns(), audio_time: audio_now })
    } else {
        None
    };
}

pub fn advance_session_frame(
    session: &mut GameSession,
    audio: &mut dyn AudioScheduler,
) -> SessionFrame {
    let times = compute_frame_times(session);
    sync_input_timestamp_anchor(session, times.audio_now);
    rebase_pre_ready_visual_times(session, times.audio_now);
    update_scratch_angle_phase(session, times.audio_now);
    let mut judgements = Vec::new();

    if session.state == PlayState::Ready && times.audio_now.0 < 0 {
        drain_pre_ready_visual_inputs(session, times.audio_now);
    } else if session.state == PlayState::Ready {
        session.state = PlayState::Playing;
    }

    if matches!(session.state, PlayState::Ready | PlayState::Playing) {
        // BGMはchart 0に間に合うようREADY中もschedule-aheadする。
        // 判定・keysound・MineはPlayingに入るまで開始しない。
        session.bgm_scheduler.schedule_until(
            &session.chart,
            &session.audio_clock,
            times.audio_schedule_until,
            session.audio_mix.master_volume
                * session.audio_mix.effective_normalization_gain()
                * session.audio_mix.bgm_volume,
            audio,
        );
    }

    if session.state == PlayState::Playing {
        sync_judge_windows(session, times.audio_now);

        if session.replay_player.is_some() && session.replay_lane_mask.is_none() {
            drain_human_inputs(session);
            judgements.extend(process_replay_inputs(session, times.audio_now));
        } else {
            if session.autoplay.as_ref().is_some_and(AutoplayController::is_full) {
                // フルオート中は人間のキー入力を判定にも視覚エフェクトにも渡さない。
                // キービームは process_autoplay_inputs 側(ノーツ処理時)で発火する。
                // (ハイスピード等のオプション操作は app 側で別途処理される)
                discard_human_inputs(session);
            } else {
                judgements.extend(process_human_inputs(session));
            }
            if session.replay_player.is_some() {
                judgements.extend(process_replay_inputs(session, times.audio_now));
            }
            judgements.extend(process_autoplay_inputs(session, times.audio_now));
            if session.autoplay.is_some() {
                apply_auto_key_release(session, times.audio_now);
            }
        }
        judgements.extend(process_mine_passes(session, times.audio_now));
        judgements.extend(process_misses(session, times.audio_now));
        update_hcn_lane_timers(session, times.audio_now);
        apply_hcn_gauge(session, times.audio_now);
        update_failed_state_from_gauge(session);
        schedule_keysounds(session, audio);
        update_recent_judgements(session, &judgements, times.audio_now);
        update_full_combo_timer(session, &judgements);
        advance_battle_opponent(session, times.audio_now);

        if should_finish(session, times.audio_now) {
            session.state = PlayState::Finished;
        }
    }
    update_gauge_max_timer(session, times.audio_now);

    let mine_hits = std::mem::take(&mut session.pending_mine_hits);
    let keysound_volumes = std::mem::take(&mut session.pending_keysound_volumes);
    let skin_events = std::mem::take(&mut session.pending_skin_events);
    SessionFrame {
        times,
        judgements,
        mine_hits,
        keysound_volumes,
        skin_events,
        state: session.state,
    }
}

fn advance_battle_opponent(session: &mut GameSession, now: TimeUs) {
    let playback_rate_percent = session.audio_clock.playback_rate_percent();
    let Some(opponent) = &mut session.battle_opponent else {
        return;
    };
    let display_uses_primary_arrangement = opponent.display_uses_primary_arrangement;

    let percent = judge_percent_at_time_for_keymode(
        opponent.chart.metadata.judge_rank_spec,
        &opponent.chart.judge_rank_events,
        now,
        opponent.key_mode,
        opponent.rule_mode,
    );
    let windows = judge_windows_for_rule_mode_and_keymode(
        opponent.base_judge_windows,
        percent,
        opponent.rule_mode,
        opponent.key_mode,
    );
    opponent
        .judge
        .set_window_set(scale_judge_windows_for_playback_rate(windows, playback_rate_percent));

    let inputs = if let Some(replay) = &mut opponent.replay_player {
        replay.poll_until(now)
    } else if let Some(autoplay) = &mut opponent.autoplay {
        autoplay.poll_until(&opponent.chart, now)
    } else {
        Vec::new()
    };
    let mut display_judgements = Vec::new();
    for input in inputs {
        match input.kind {
            InputKind::Press => {
                opponent.lane_keyon_started_at[input.lane.index()] = Some(input.time)
            }
            InputKind::Release => opponent.lane_keyon_started_at[input.lane.index()] = None,
        }
        let outcome = opponent.judge.process_input(&opponent.chart, input);
        display_judgements.extend(apply_battle_opponent_outcome(opponent, outcome));
    }
    let mine_outcome =
        opponent.judge.process_mine_passes(&opponent.chart, now, &opponent.lane_keyon_started_at);
    display_judgements.extend(apply_battle_opponent_outcome(opponent, mine_outcome));
    let miss_outcome = opponent.judge.process_misses(&opponent.chart, now);
    display_judgements.extend(apply_battle_opponent_outcome(opponent, miss_outcome));

    for display in &mut display_judgements {
        let source_lane = if display_uses_primary_arrangement {
            display
                .judgement
                .note_id
                .and_then(|note_id| session.chart.note_by_id(note_id))
                .map_or(display.judgement.lane, |note| note.lane)
        } else {
            display.judgement.lane
        };
        display.judgement.lane = second_player_lane(source_lane);
        display.judgement.affects_score = false;
        push_skin_runtime_event(
            session,
            SkinRuntimeEventKind::Judgement(display.judgement.clone()),
        );
    }
    session
        .recent_judgements
        .extend(display_judgements.iter().map(|display| display.judgement.clone()));
    session.recent_display_judgements.extend(display_judgements);
}

fn apply_battle_opponent_outcome(
    opponent: &mut BattleOpponentSession,
    outcome: JudgeOutcome,
) -> Vec<DisplayJudgementEvent> {
    let mut display_judgements = Vec::with_capacity(outcome.events.len());
    for event in outcome.events {
        if event.affects_score {
            opponent.score.apply(&event);
            opponent.gauge.apply_judge(event.judge, 1.0);
        }
        display_judgements
            .push(DisplayJudgementEvent { judgement: event, combo: opponent.score.combo });
    }
    for mine in outcome.mine_hits {
        opponent.gauge.apply_mine(mine.damage);
    }
    display_judgements
}

fn second_player_lane(lane: Lane) -> Lane {
    match lane {
        Lane::Scratch => Lane::Scratch2,
        Lane::Key1 => Lane::Key8,
        Lane::Key2 => Lane::Key9,
        Lane::Key3 => Lane::Key10,
        Lane::Key4 => Lane::Key11,
        Lane::Key5 => Lane::Key12,
        Lane::Key6 => Lane::Key13,
        Lane::Key7 => Lane::Key14,
        lane => lane,
    }
}

pub(super) fn update_full_combo_timer(session: &mut GameSession, judgements: &[JudgementEvent]) {
    update_opponent_full_combo_timer(session, judgements);
    if session.full_combo_started_at.is_some()
        || session.scored_total_notes == 0
        || session.score.past_notes < session.scored_total_notes
        || session.score.combo < session.scored_total_notes
    {
        return;
    }
    session.full_combo_started_at = judgements
        .iter()
        .rev()
        .find(|event| event.affects_score && event.note_id.is_some())
        .map(|event| event.time)
        .or_else(|| Some(session.audio_clock.now()));
}

fn update_opponent_full_combo_timer(session: &mut GameSession, judgements: &[JudgementEvent]) {
    let Some(score) = &session.opponent_score else {
        return;
    };
    if session.opponent_full_combo_started_at.is_some()
        || session.scored_total_notes == 0
        || score.past_notes < session.scored_total_notes
        || score.combo < session.scored_total_notes
    {
        return;
    }
    session.opponent_full_combo_started_at = judgements
        .iter()
        .rev()
        .find(|event| session.display_only_lane_mask[event.lane.index()] && event.note_id.is_some())
        .map(|event| event.time)
        .or_else(|| Some(session.audio_clock.now()));
}

pub fn should_finish(session: &GameSession, audio_now: TimeUs) -> bool {
    session.judge.is_exhausted(&session.chart)
        && session.bgm_scheduler.is_done(&session.chart)
        && audio_now.0 > session.chart.end_time.0.saturating_add(SESSION_END_MARGIN_US)
}

/// 最終ノーツに対する Poor / Empty Poor / Mine の SLOW 側受付が終了し、
/// これ以上スコアが変化しない状態かを返す。
pub fn result_is_settled(session: &GameSession, audio_now: TimeUs) -> bool {
    let result_settle_at =
        session.chart.end_time.0.saturating_add(session.judge.window_set.result_settle_margin_us());
    session.judge.is_exhausted(&session.chart) && audio_now.0 > result_settle_at
}
use super::*;
