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

        // ノーツの押下有無に関わらず、譜面の生タイミングでキー音を鳴らす。
        // 入力オフセット・表示オフセットは適用しない。0 なら機能自体を無効化する。
        if session.audio_mix.auto_key_volume > 0.0 {
            session.auto_keysound_scheduler.schedule_until(
                &session.chart,
                &session.audio_clock,
                times.audio_schedule_until,
                session.audio_mix.master_volume
                    * session.audio_mix.effective_normalization_gain()
                    * session.audio_mix.auto_key_volume,
                audio,
            );
        }
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
    // Seed-only providers can reproduce the lane arrangement but do not
    // publish input timing. Keep the opponent unjudged instead of fabricating
    // misses or autoplay data.
    if opponent.replay_player.is_none() {
        return;
    }

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

    let inputs = opponent.replay_player.as_mut().expect("checked above").poll_until(now);
    for input in inputs {
        match input.kind {
            InputKind::Press => {
                opponent.lane_keyon_started_at[input.lane.index()] = Some(input.time)
            }
            InputKind::Release => opponent.lane_keyon_started_at[input.lane.index()] = None,
        }
        let outcome = opponent.judge.process_input(&opponent.chart, input);
        apply_battle_opponent_outcome(opponent, outcome);
    }
    let mine_outcome =
        opponent.judge.process_mine_passes(&opponent.chart, now, &opponent.lane_keyon_started_at);
    apply_battle_opponent_outcome(opponent, mine_outcome);
    let miss_outcome = opponent.judge.process_misses(&opponent.chart, now);
    apply_battle_opponent_outcome(opponent, miss_outcome);
}

fn apply_battle_opponent_outcome(opponent: &mut BattleOpponentSession, outcome: JudgeOutcome) {
    for event in outcome.events {
        if event.affects_score {
            opponent.score.apply(&event);
            opponent.gauge.apply_judge(event.judge, 1.0);
        }
    }
    for mine in outcome.mine_hits {
        opponent.gauge.apply_mine(mine.damage);
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
