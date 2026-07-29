pub fn sync_judge_windows(session: &mut GameSession, now: TimeUs) {
    let percent = judge_percent_at_time_for_keymode(
        session.chart.metadata.judge_rank_spec,
        &session.chart.judge_rank_events,
        now,
        session.primary_key_mode,
        session.rule_mode,
    );
    session.judge.set_window_set(judge_windows_for_rule_mode_and_keymode(
        session.base_judge_windows,
        percent,
        session.rule_mode,
        session.primary_key_mode,
    ));
}

fn sync_input_timestamp_anchor(session: &mut GameSession, audio_now: TimeUs) {
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

fn update_full_combo_timer(session: &mut GameSession, judgements: &[JudgementEvent]) {
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
        && audio_now.0 > session.chart.end_time.0 + SESSION_END_MARGIN_US
}
